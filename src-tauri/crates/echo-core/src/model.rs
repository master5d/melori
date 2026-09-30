use anyhow::Result;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tar::Archive;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum EngineType {
    Whisper,
    Parakeet,
    Moonshine,
    MoonshineStreaming,
    SenseVoice,
    GigaAM,
    Canary,
    Cohere,
    PiperTts,
    SupertonicTts,
    KittenTts,
}

impl EngineType {
    pub fn is_asr(&self) -> bool {
        !matches!(
            self,
            EngineType::PiperTts | EngineType::SupertonicTts | EngineType::KittenTts
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelFileEntry {
    pub path: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub size_mb: u64,
    pub is_downloaded: bool,
    pub is_downloading: bool,
    pub partial_size: u64,
    pub is_directory: bool,
    pub engine_type: EngineType,
    pub accuracy_score: f32,        // 0.0 to 1.0, higher is more accurate
    pub speed_score: f32,           // 0.0 to 1.0, higher is faster
    pub supports_translation: bool, // Whether the model supports translating to English
    pub is_recommended: bool,       // Whether this is the recommended model for new users
    pub supported_languages: Vec<String>, // Languages this model can transcribe
    pub supports_language_selection: bool, // Whether the user can explicitly pick a language
    pub is_custom: bool,            // Whether this is a user-provided custom model
    #[serde(default)]
    pub files: Vec<ModelFileEntry>, // Per-file (path/url/sha256) pins for multi-file layouts
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// RAII guard that cleans up download state (`is_downloading` flag and cancel flag)
/// when dropped, unless explicitly disarmed. This ensures consistent cleanup on
/// every error path without requiring manual cleanup at each `?` or `return Err`.
struct DownloadCleanup<'a> {
    available_models: &'a Mutex<HashMap<String, ModelInfo>>,
    cancel_flags: &'a Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    model_id: String,
    disarmed: bool,
}

impl<'a> Drop for DownloadCleanup<'a> {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(self.model_id.as_str()) {
                model.is_downloading = false;
            }
        }
        self.cancel_flags.lock().unwrap().remove(&self.model_id);
    }
}

/// Resolves the on-disk target for a per-file entry of a multi-file model
/// (`ModelInfo.files`), rejecting any `entry_path` that could escape the
/// model's own directory (path traversal / absolute-path supply-chain guard).
///
/// Rejects: an empty path, any `..` (`Component::ParentDir`) component, and
/// any absolute-path component (`Component::RootDir` / `Component::Prefix`,
/// which together catch both `/abs/path` and Windows `C:/abs`).
fn multi_file_target(models_dir: &Path, filename: &str, entry_path: &str) -> Result<PathBuf> {
    if entry_path.is_empty() {
        return Err(anyhow::anyhow!("empty file path in model manifest entry"));
    }
    let rel = Path::new(entry_path);
    for component in rel.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow::anyhow!(
                    "unsafe path component in model manifest entry: {}",
                    entry_path
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(models_dir.join(filename).join(rel))
}

/// True when every file listed in `info.files` exists on disk under its
/// sanitized target path. Used in place of the single-file `.exists()` check
/// for multi-file model layouts (e.g. `supertonic-3`).
fn multi_file_complete(models_dir: &Path, info: &ModelInfo) -> bool {
    info.files.iter().all(|f| {
        multi_file_target(models_dir, &info.filename, &f.path)
            .map(|p| p.exists())
            .unwrap_or(false)
    })
}

pub struct ModelManager {
    resource_resolver: std::sync::Arc<dyn Fn(&str) -> Option<std::path::PathBuf> + Send + Sync>,
    events: std::sync::Arc<dyn crate::EventSink>,
    settings: std::sync::Arc<dyn crate::SettingsAccess>,
    models_dir: PathBuf,
    available_models: Mutex<HashMap<String, ModelInfo>>,
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    extracting_models: Arc<Mutex<HashSet<String>>>,
    #[cfg_attr(not(feature = "diarization"), allow(dead_code))]
    diarization_ensured: Arc<AtomicBool>,
}

impl ModelManager {
    pub fn new(
        models_dir: std::path::PathBuf,
        resource_resolver: std::sync::Arc<dyn Fn(&str) -> Option<std::path::PathBuf> + Send + Sync>,
        events: std::sync::Arc<dyn crate::EventSink>,
        settings: std::sync::Arc<dyn crate::SettingsAccess>,
    ) -> Result<Self> {
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        // Build the predefined model set from the embedded manifest. The manifest
        // (src-tauri/resources/models.json) is the single source of truth for
        // download URLs, checksums, and metadata; see model_manifest.rs.
        let manifest = crate::model_manifest::load_manifest()
            .map_err(|e| anyhow::anyhow!("Failed to load model manifest: {}", e))?;

        let mut available_models: HashMap<String, ModelInfo> = HashMap::new();
        for entry in &manifest.models {
            available_models.insert(entry.id.clone(), entry.to_model_info());
        }

        // Auto-discover custom Whisper models (.bin files) in the models directory
        if let Err(e) = Self::discover_custom_whisper_models(&models_dir, &mut available_models) {
            warn!("Failed to discover custom models: {}", e);
        }

        let manager = Self {
            resource_resolver,
            events,
            settings,
            models_dir,
            available_models: Mutex::new(available_models),
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            extracting_models: Arc::new(Mutex::new(HashSet::new())),
            diarization_ensured: Arc::new(AtomicBool::new(false)),
        };

        // Migrate any bundled models to user directory
        manager.migrate_bundled_models()?;

        // Migrate GigaAM from single-file to directory format
        manager.migrate_gigaam_to_directory()?;

        // Check which models are already downloaded
        manager.update_download_status()?;

        // Auto-select a model if none is currently selected
        manager.auto_select_model_if_needed()?;

        Ok(manager)
    }

    pub fn get_available_models(&self) -> Vec<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        models
            .values()
            .filter(|m| m.engine_type.is_asr())
            .cloned()
            .collect()
    }

    pub fn get_model_info(&self, model_id: &str) -> Option<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        models.get(model_id).cloned()
    }

    fn migrate_bundled_models(&self) -> Result<()> {
        // Check for bundled models and copy them to user directory
        let bundled_models = ["ggml-small.bin"]; // Add other bundled models here if any

        for filename in &bundled_models {
            let bundled_path = (self.resource_resolver)(&format!("resources/models/{}", filename));

            if let Some(bundled_path) = bundled_path {
                if bundled_path.exists() {
                    let user_path = self.models_dir.join(filename);

                    // Only copy if user doesn't already have the model
                    if !user_path.exists() {
                        info!("Migrating bundled model {} to user directory", filename);
                        fs::copy(&bundled_path, &user_path)?;
                        info!("Successfully migrated {}", filename);
                    }
                }
            }
        }

        Ok(())
    }

    /// Migrate GigaAM from the old single-file format (giga-am-v3.int8.onnx)
    /// to the new directory format (giga-am-v3-int8/model.int8.onnx + vocab.txt).
    /// This was required by the transcribe-rs 0.3.x upgrade.
    fn migrate_gigaam_to_directory(&self) -> Result<()> {
        let old_file = self.models_dir.join("giga-am-v3.int8.onnx");
        let new_dir = self.models_dir.join("giga-am-v3-int8");

        if !old_file.exists() || new_dir.exists() {
            return Ok(());
        }

        info!("Migrating GigaAM from single-file to directory format");

        let vocab_path =
            (self.resource_resolver)("resources/models/gigaam_vocab.txt").ok_or_else(|| {
                anyhow::anyhow!(
                    "resource not found: {}",
                    "resources/models/gigaam_vocab.txt"
                )
            })?;

        info!(
            "Resolved vocab path: {:?} (exists: {})",
            vocab_path,
            vocab_path.exists()
        );
        info!("Old file: {:?} (exists: {})", old_file, old_file.exists());
        info!("New dir: {:?} (exists: {})", new_dir, new_dir.exists());

        fs::create_dir_all(&new_dir)?;
        fs::rename(&old_file, new_dir.join("model.int8.onnx"))?;
        fs::copy(&vocab_path, new_dir.join("vocab.txt"))?;

        // Clean up old partial file if it exists
        let old_partial = self.models_dir.join("giga-am-v3.int8.onnx.partial");
        if old_partial.exists() {
            let _ = fs::remove_file(&old_partial);
        }

        info!("GigaAM migration complete");
        Ok(())
    }

    fn update_download_status(&self) -> Result<()> {
        let mut models = self.available_models.lock().unwrap();

        for model in models.values_mut() {
            if !model.files.is_empty() {
                // Multi-file layout (e.g. supertonic-3): "downloaded" means every
                // pinned file is present on disk, not just the top-level model dir.
                model.is_downloaded = multi_file_complete(&self.models_dir, model);
                model.is_downloading = false;
                model.partial_size = 0;
            } else if model.is_directory {
                // For directory-based models, check if the directory exists
                let model_path = self.models_dir.join(&model.filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));
                let extracting_path = self
                    .models_dir
                    .join(format!("{}.extracting", &model.filename));

                // Clean up any leftover .extracting directories from interrupted extractions
                // But only if this model is NOT currently being extracted
                let is_currently_extracting = {
                    let extracting = self.extracting_models.lock().unwrap();
                    extracting.contains(&model.id)
                };
                if extracting_path.exists() && !is_currently_extracting {
                    warn!("Cleaning up interrupted extraction for model: {}", model.id);
                    let _ = fs::remove_dir_all(&extracting_path);
                }

                model.is_downloaded = model_path.exists() && model_path.is_dir();
                model.is_downloading = false;

                // Get partial file size if it exists (for the .tar.gz being downloaded)
                if partial_path.exists() {
                    model.partial_size = partial_path.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    model.partial_size = 0;
                }
            } else {
                // For file-based models (existing logic)
                let model_path = self.models_dir.join(&model.filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));

                model.is_downloaded = model_path.exists();
                model.is_downloading = false;

                // Get partial file size if it exists
                if partial_path.exists() {
                    model.partial_size = partial_path.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    model.partial_size = 0;
                }
            }
        }

        Ok(())
    }

    fn auto_select_model_if_needed(&self) -> Result<()> {
        let mut settings = self.settings.get();

        // Clear stale selection: selected model is set but doesn't exist
        // in available_models (e.g. deleted custom model file)
        if !settings.selected_model.is_empty() {
            let models = self.available_models.lock().unwrap();
            let exists = models
                .get(&settings.selected_model)
                .map(|m| m.engine_type.is_asr())
                .unwrap_or(false);
            drop(models);

            if !exists {
                info!(
                    "Selected model '{}' not found in available models, clearing selection",
                    settings.selected_model
                );
                settings.selected_model = String::new();
                self.settings.set(settings.clone());
            }
        }

        // If no model is selected, pick the first downloaded one
        if settings.selected_model.is_empty() {
            // Find the first available (downloaded) ASR model
            let models = self.available_models.lock().unwrap();
            if let Some(available_model) = models
                .values()
                .find(|model| model.is_downloaded && model.engine_type.is_asr())
            {
                info!(
                    "Auto-selecting model: {} ({})",
                    available_model.id, available_model.name
                );

                // Update settings with the selected model
                let mut updated_settings = settings;
                updated_settings.selected_model = available_model.id.clone();
                self.settings.set(updated_settings);

                info!("Successfully auto-selected model: {}", available_model.id);
            }
        }

        Ok(())
    }

    /// Discover custom Whisper models (.bin files) in the models directory.
    /// Skips files that match predefined model filenames.
    fn discover_custom_whisper_models(
        models_dir: &Path,
        available_models: &mut HashMap<String, ModelInfo>,
    ) -> Result<()> {
        if !models_dir.exists() {
            return Ok(());
        }

        // Collect filenames of predefined Whisper file-based models to skip
        let predefined_filenames: HashSet<String> = available_models
            .values()
            .filter(|m| matches!(m.engine_type, EngineType::Whisper) && !m.is_directory)
            .map(|m| m.filename.clone())
            .collect();

        // Scan models directory for .bin files
        for entry in fs::read_dir(models_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Only process .bin files (not directories)
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip hidden files
            if filename.starts_with('.') {
                continue;
            }

            // Only process .bin files (Whisper GGML format).
            // This also excludes .partial downloads (e.g., "model.bin.partial").
            // If we add discovery for other formats, add a .partial check before this filter.
            if !filename.ends_with(".bin") {
                continue;
            }

            // Skip predefined model files
            if predefined_filenames.contains(&filename) {
                continue;
            }

            // Generate model ID from filename (remove .bin extension)
            let model_id = filename.trim_end_matches(".bin").to_string();

            // Skip if model ID already exists (shouldn't happen, but be safe)
            if available_models.contains_key(&model_id) {
                continue;
            }

            // Generate display name: replace - and _ with space, capitalize words
            let display_name = model_id
                .replace(['-', '_'], " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            // Get file size in MB
            let size_mb = match path.metadata() {
                Ok(meta) => meta.len() / (1024 * 1024),
                Err(e) => {
                    warn!("Failed to get metadata for {}: {}", filename, e);
                    0
                }
            };

            info!(
                "Discovered custom Whisper model: {} ({}, {} MB)",
                model_id, filename, size_mb
            );

            available_models.insert(
                model_id.clone(),
                ModelInfo {
                    id: model_id,
                    name: display_name,
                    description: "Not officially supported".to_string(),
                    filename,
                    url: None,    // Custom models have no download URL
                    sha256: None, // Custom models skip verification
                    size_mb,
                    is_downloaded: true, // Already present on disk
                    is_downloading: false,
                    partial_size: 0,
                    is_directory: false,
                    engine_type: EngineType::Whisper,
                    accuracy_score: 0.0, // Sentinel: UI hides score bars when both are 0
                    speed_score: 0.0,
                    supports_translation: false,
                    is_recommended: false,
                    supported_languages: vec![],
                    supports_language_selection: true,
                    is_custom: true,
                    files: vec![],
                },
            );
        }

        Ok(())
    }

    /// Verifies the SHA256 of `path` against `expected_sha256` (if provided).
    /// On mismatch or read error the partial file is deleted and an error is returned,
    /// so the next download attempt always starts from a clean state.
    /// When `expected_sha256` is `None` (custom user models) verification is skipped.
    fn verify_sha256(path: &Path, expected_sha256: Option<&str>, model_id: &str) -> Result<()> {
        let Some(expected) = expected_sha256 else {
            return Ok(());
        };
        match Self::compute_sha256(path) {
            Ok(actual) if actual == expected => {
                info!("SHA256 verified for model {}", model_id);
                Ok(())
            }
            Ok(actual) => {
                warn!(
                    "SHA256 mismatch for model {}: expected {}, got {}",
                    model_id, expected, actual
                );
                let _ = fs::remove_file(path);
                Err(anyhow::anyhow!(
                    "Download verification failed for model {}: file is corrupt. Please retry.",
                    model_id
                ))
            }
            Err(e) => {
                let _ = fs::remove_file(path);
                Err(anyhow::anyhow!(
                    "Failed to verify download for model {}: {}. Please retry.",
                    model_id,
                    e
                ))
            }
        }
    }

    /// Computes the SHA256 hex digest of a file, reading in 64KB chunks to handle large models.
    fn compute_sha256(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub async fn download_model(&self, model_id: &str) -> Result<()> {
        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.files.is_empty() {
            return self.download_model_multi(model_id, &model_info).await;
        }

        let manifest_entry = crate::model_manifest::load_manifest()?
            .models
            .into_iter()
            .find(|entry| entry.id == model_id)
            .ok_or_else(|| anyhow::anyhow!("Model {} is not downloadable", model_id))?;
        let mirror_url = self.settings.get().model_mirror_url;
        let url = crate::model_manifest::resolve_download_url(&manifest_entry, &mirror_url)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Model '{}' is unavailable: configure a model mirror URL in Settings",
                    model_id
                )
            })?;
        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        // Don't download if complete version already exists
        if model_path.exists() {
            // Clean up any partial file that might exist
            if partial_path.exists() {
                let _ = fs::remove_file(&partial_path);
            }
            self.update_download_status()?;
            return Ok(());
        }

        // Check if we have a partial download to resume
        let mut resume_from = if partial_path.exists() {
            let size = partial_path.metadata()?.len();
            info!("Resuming download of model {} from byte {}", model_id, size);
            size
        } else {
            info!("Starting fresh download of model {} from {}", model_id, url);
            0
        };

        // Mark as downloading
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }

        // Create cancellation flag for this download
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        // Guard ensures is_downloading and cancel_flags are cleaned up on every
        // error path. Disarmed only on success (which sets is_downloaded = true).
        let mut cleanup = DownloadCleanup {
            available_models: &self.available_models,
            cancel_flags: &self.cancel_flags,
            model_id: model_id.to_string(),
            disarmed: false,
        };

        // Create HTTP client with range request for resuming
        let client = reqwest::Client::new();
        let mut request = client.get(&url);

        if resume_from > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from));
        }

        let mut response = request.send().await?;

        // If we tried to resume but server returned 200 (not 206 Partial Content),
        // the server doesn't support range requests. Delete partial file and restart
        // fresh to avoid file corruption (appending full file to partial).
        if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
            warn!(
                "Server doesn't support range requests for model {}, restarting download",
                model_id
            );
            drop(response);
            let _ = fs::remove_file(&partial_path);

            // Reset resume_from since we're starting fresh
            resume_from = 0;

            // Restart download without range header
            response = client.get(&url).send().await?;
        }

        // Check for success or partial content status
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(anyhow::anyhow!(
                "Failed to download model: HTTP {}",
                response.status()
            ));
        }

        let total_size = if resume_from > 0 {
            // For resumed downloads, add the resume point to content length
            resume_from + response.content_length().unwrap_or(0)
        } else {
            response.content_length().unwrap_or(0)
        };

        let mut downloaded = resume_from;
        let mut stream = response.bytes_stream();

        // Open file for appending if resuming, or create new if starting fresh
        let mut file = if resume_from > 0 {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&partial_path)?
        } else {
            std::fs::File::create(&partial_path)?
        };

        // Emit initial progress
        let initial_progress = DownloadProgress {
            model_id: model_id.to_string(),
            downloaded,
            total: total_size,
            percentage: if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            },
        };
        self.events.emit(
            "model-download-progress",
            serde_json::to_value(&initial_progress).unwrap_or_default(),
        );

        // Throttle progress events to max 10/sec (100ms intervals)
        let mut last_emit = Instant::now();
        let throttle_duration = Duration::from_millis(100);

        // Download with progress
        while let Some(chunk) = stream.next().await {
            // Check if download was cancelled
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                info!("Download cancelled for: {}", model_id);
                // Keep partial file for resume functionality.
                // Guard handles is_downloading + cancel_flags cleanup on drop.
                return Ok(());
            }

            let chunk = chunk?;

            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            // Emit progress event (throttled to avoid UI freeze)
            if last_emit.elapsed() >= throttle_duration {
                let progress = DownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded,
                    total: total_size,
                    percentage,
                };
                self.events.emit(
                    "model-download-progress",
                    serde_json::to_value(&progress).unwrap_or_default(),
                );
                last_emit = Instant::now();
            }
        }

        // Emit final progress to ensure 100% is shown
        let final_progress = DownloadProgress {
            model_id: model_id.to_string(),
            downloaded,
            total: total_size,
            percentage: if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                100.0
            },
        };
        self.events.emit(
            "model-download-progress",
            serde_json::to_value(&final_progress).unwrap_or_default(),
        );

        file.flush()?;
        drop(file); // Ensure file is closed before moving

        // Verify downloaded file size matches expected size
        if total_size > 0 {
            let actual_size = partial_path.metadata()?.len();
            if actual_size != total_size {
                // Download is incomplete/corrupted - delete partial and return error
                let _ = fs::remove_file(&partial_path);
                return Err(anyhow::anyhow!(
                    "Download incomplete: expected {} bytes, got {} bytes",
                    total_size,
                    actual_size
                ));
            }
        }

        // Verify SHA256 checksum. Runs in a blocking thread so the async executor is not
        // stalled while hashing large model files (up to 1.6 GB). On failure the partial
        // is deleted inside verify_sha256 so the next attempt always starts fresh.
        self.events.emit(
            "model-verification-started",
            serde_json::to_value(model_id).unwrap_or_default(),
        );
        info!("Verifying SHA256 for model {}...", model_id);
        let verify_path = partial_path.clone();
        let verify_expected = model_info.sha256.clone();
        let verify_model_id = model_id.to_string();
        let verify_result = tokio::task::spawn_blocking(move || {
            Self::verify_sha256(&verify_path, verify_expected.as_deref(), &verify_model_id)
        })
        .await
        .map_err(|e| anyhow::anyhow!("SHA256 task panicked: {}", e))?;
        verify_result?;
        self.events.emit(
            "model-verification-completed",
            serde_json::to_value(model_id).unwrap_or_default(),
        );

        // Handle directory-based models (extract tar.gz) vs file-based models
        if model_info.is_directory {
            // Track that this model is being extracted
            {
                let mut extracting = self.extracting_models.lock().unwrap();
                extracting.insert(model_id.to_string());
            }

            // Emit extraction started event
            self.events.emit(
                "model-extraction-started",
                serde_json::to_value(model_id).unwrap_or_default(),
            );
            info!("Extracting archive for directory-based model: {}", model_id);

            // Use a temporary extraction directory to ensure atomic operations
            let temp_extract_dir = self
                .models_dir
                .join(format!("{}.extracting", &model_info.filename));
            let final_model_dir = self.models_dir.join(&model_info.filename);

            // Clean up any previous incomplete extraction
            if temp_extract_dir.exists() {
                let _ = fs::remove_dir_all(&temp_extract_dir);
            }

            // Create temporary extraction directory
            fs::create_dir_all(&temp_extract_dir)?;

            // Sniff the archive's magic bytes to pick the decoder. Official sherpa-onnx
            // bundles on k2-fsa GitHub releases are .tar.bz2; whisper/own-R2 use .tar.gz.
            let mut magic = [0u8; 3];
            {
                use std::io::Read as _;
                let mut probe = File::open(&partial_path)?;
                let _ = probe.read(&mut magic);
            }
            let archive_file = File::open(&partial_path)?;
            let reader: Box<dyn std::io::Read> = if magic == [0x42, 0x5a, 0x68] {
                Box::new(bzip2::read::BzDecoder::new(archive_file))
            } else {
                Box::new(GzDecoder::new(archive_file))
            };
            let mut archive = Archive::new(reader);

            // Extract to the temporary directory first
            archive.unpack(&temp_extract_dir).map_err(|e| {
                let error_msg = format!("Failed to extract archive: {}", e);
                // Clean up failed extraction
                let _ = fs::remove_dir_all(&temp_extract_dir);
                // Delete the corrupt partial file so the next download attempt starts fresh
                // instead of resuming from a broken archive (issue #858).
                let _ = fs::remove_file(&partial_path);
                // Remove from extracting set
                {
                    let mut extracting = self.extracting_models.lock().unwrap();
                    extracting.remove(model_id);
                }
                self.events.emit(
                    "model-extraction-failed",
                    serde_json::json!({
                        "model_id": model_id,
                        "error": error_msg
                    }),
                );
                anyhow::anyhow!(error_msg)
            })?;

            // Find the actual extracted directory (archive might have a nested structure)
            let extracted_dirs: Vec<_> = fs::read_dir(&temp_extract_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                .collect();

            if extracted_dirs.len() == 1 {
                // Single directory extracted, move it to the final location
                let source_dir = extracted_dirs[0].path();
                if final_model_dir.exists() {
                    fs::remove_dir_all(&final_model_dir)?;
                }
                fs::rename(&source_dir, &final_model_dir)?;
                // Clean up temp directory
                let _ = fs::remove_dir_all(&temp_extract_dir);
            } else {
                // Multiple items or no directories, rename the temp directory itself
                if final_model_dir.exists() {
                    fs::remove_dir_all(&final_model_dir)?;
                }
                fs::rename(&temp_extract_dir, &final_model_dir)?;
            }

            info!("Successfully extracted archive for model: {}", model_id);
            // Remove from extracting set
            {
                let mut extracting = self.extracting_models.lock().unwrap();
                extracting.remove(model_id);
            }
            // Emit extraction completed event
            self.events.emit(
                "model-extraction-completed",
                serde_json::to_value(model_id).unwrap_or_default(),
            );

            // Remove the downloaded tar.gz file
            let _ = fs::remove_file(&partial_path);
        } else {
            // Move partial file to final location for file-based models
            fs::rename(&partial_path, &model_path)?;
        }

        // Disarm the guard — success path does its own cleanup because it
        // additionally sets is_downloaded = true.
        cleanup.disarmed = true;
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.is_downloaded = true;
                model.partial_size = 0;
            }
        }
        self.cancel_flags.lock().unwrap().remove(model_id);

        // Emit completion event
        self.events.emit(
            "model-download-complete",
            serde_json::to_value(model_id).unwrap_or_default(),
        );

        info!(
            "Successfully downloaded model {} to {:?}",
            model_id, model_path
        );

        Ok(())
    }

    /// Downloads every file of a multi-file model (`ModelInfo.files`) into
    /// `models_dir/<filename>/<entry.path>`. Mirrors the single-file path's
    /// bookkeeping (is_downloading flag, cancel flag, `DownloadCleanup`,
    /// throttled `DownloadProgress` events) but loops over the pinned files:
    /// each file is downloaded to `<target>.partial`, sha256-verified with the
    /// same verifier as the single-file path, then renamed into place.
    /// Already-present files with a matching sha256 are skipped, so a failed
    /// run restarts the current file, not the whole bundle (no cross-session
    /// resume of per-file partials — acceptable per spec).
    async fn download_model_multi(&self, model_id: &str, model_info: &ModelInfo) -> Result<()> {
        // Sanitize every entry path up-front, before any network or disk I/O
        // (supply-chain guard: a poisoned manifest must not be able to write
        // outside the model's own directory).
        let mut targets: Vec<(&ModelFileEntry, PathBuf)> =
            Vec::with_capacity(model_info.files.len());
        for entry in &model_info.files {
            let target = multi_file_target(&self.models_dir, &model_info.filename, &entry.path)?;
            targets.push((entry, target));
        }

        // Don't download if every file is already in place.
        if multi_file_complete(&self.models_dir, model_info) {
            self.update_download_status()?;
            return Ok(());
        }

        info!(
            "Starting multi-file download of model {} ({} files)",
            model_id,
            targets.len()
        );

        // Mark as downloading
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }

        // Create cancellation flag for this download
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        // Guard ensures is_downloading and cancel_flags are cleaned up on every
        // error path. Disarmed only on success (which sets is_downloaded = true).
        let mut cleanup = DownloadCleanup {
            available_models: &self.available_models,
            cancel_flags: &self.cancel_flags,
            model_id: model_id.to_string(),
            disarmed: false,
        };

        let client = reqwest::Client::new();

        // Progress total = sum of per-file content lengths (HEAD pre-pass over
        // files not yet on disk); falls back to the size_mb approximation when
        // a server does not report a length — same spirit as the single-file
        // path's content_length().unwrap_or(0), without losing the percentage.
        let mut total: u64 = 0;
        let mut lengths_known = true;
        for (entry, target) in &targets {
            if cancel_flag.load(Ordering::Relaxed) {
                info!("Download cancelled for: {}", model_id);
                return Ok(());
            }
            if target.exists() {
                total += target.metadata().map(|m| m.len()).unwrap_or(0);
                continue;
            }
            match client.head(&entry.url).send().await {
                Ok(resp) if resp.status().is_success() => match resp.content_length() {
                    Some(len) => total += len,
                    None => {
                        lengths_known = false;
                        break;
                    }
                },
                _ => {
                    lengths_known = false;
                    break;
                }
            }
        }
        if !lengths_known {
            total = model_info.size_mb.saturating_mul(1024 * 1024);
        }

        let emit_progress = |downloaded: u64| {
            let progress = DownloadProgress {
                model_id: model_id.to_string(),
                downloaded,
                total,
                percentage: if total > 0 {
                    ((downloaded as f64 / total as f64) * 100.0).min(100.0)
                } else {
                    0.0
                },
            };
            self.events.emit(
                "model-download-progress",
                serde_json::to_value(&progress).unwrap_or_default(),
            );
        };

        let mut downloaded: u64 = 0;
        // Throttle progress events to max 10/sec (100ms intervals)
        let mut last_emit = Instant::now();
        let throttle_duration = Duration::from_millis(100);

        // Emit initial progress
        emit_progress(downloaded);

        for (entry, target) in &targets {
            if cancel_flag.load(Ordering::Relaxed) {
                info!("Download cancelled for: {}", model_id);
                // Guard handles is_downloading + cancel_flags cleanup on drop.
                return Ok(());
            }

            // Skip files already downloaded and verified (e.g. previous session).
            if target.exists() {
                let existing = target.clone();
                let hash = tokio::task::spawn_blocking(move || Self::compute_sha256(&existing))
                    .await
                    .map_err(|e| anyhow::anyhow!("SHA256 task panicked: {}", e))?;
                match hash {
                    Ok(actual) if actual == entry.sha256 => {
                        downloaded += target.metadata().map(|m| m.len()).unwrap_or(0);
                        emit_progress(downloaded);
                        continue;
                    }
                    _ => {
                        warn!(
                            "Re-downloading {} for model {}: existing file failed verification",
                            entry.path, model_id
                        );
                        let _ = fs::remove_file(target);
                    }
                }
            }

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let file_name = target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            let partial = target.with_file_name(format!("{}.partial", file_name));
            // No cross-session resume for per-file partials: restart this file.
            if partial.exists() {
                let _ = fs::remove_file(&partial);
            }

            let response = client.get(&entry.url).send().await?;
            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to download {} for model {}: HTTP {}",
                    entry.path,
                    model_id,
                    response.status()
                ));
            }

            let mut stream = response.bytes_stream();
            let mut file = File::create(&partial)?;

            while let Some(chunk) = stream.next().await {
                // Check if download was cancelled
                if cancel_flag.load(Ordering::Relaxed) {
                    drop(file);
                    // Per-file partials are not resumed — remove to keep a clean state.
                    let _ = fs::remove_file(&partial);
                    info!("Download cancelled for: {}", model_id);
                    // Guard handles is_downloading + cancel_flags cleanup on drop.
                    return Ok(());
                }

                let chunk = chunk?;
                file.write_all(&chunk)?;
                downloaded += chunk.len() as u64;

                // Emit progress event (throttled to avoid UI freeze)
                if last_emit.elapsed() >= throttle_duration {
                    emit_progress(downloaded);
                    last_emit = Instant::now();
                }
            }

            file.flush()?;
            drop(file); // Ensure file is closed before verify + rename

            // Per-file SHA256 verification with the same verifier as the
            // single-file path (deletes the partial and errors on mismatch).
            // Runs in a blocking thread so the async executor is not stalled.
            self.events.emit(
                "model-verification-started",
                serde_json::to_value(model_id).unwrap_or_default(),
            );
            let verify_path = partial.clone();
            let verify_expected = entry.sha256.clone();
            let verify_label = format!("{} ({})", model_id, entry.path);
            let verify_result = tokio::task::spawn_blocking(move || {
                Self::verify_sha256(&verify_path, Some(&verify_expected), &verify_label)
            })
            .await
            .map_err(|e| anyhow::anyhow!("SHA256 task panicked: {}", e))?;
            verify_result?;
            self.events.emit(
                "model-verification-completed",
                serde_json::to_value(model_id).unwrap_or_default(),
            );

            fs::rename(&partial, target)?;
        }

        // Emit final progress to ensure 100% is shown
        emit_progress(if total > 0 { total } else { downloaded });

        // Disarm the guard — success path does its own cleanup because it
        // additionally sets is_downloaded = true.
        cleanup.disarmed = true;
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.is_downloaded = true;
                model.partial_size = 0;
            }
        }
        self.cancel_flags.lock().unwrap().remove(model_id);

        // Emit completion event
        self.events.emit(
            "model-download-complete",
            serde_json::to_value(model_id).unwrap_or_default(),
        );

        info!(
            "Successfully downloaded multi-file model {} ({} files)",
            model_id,
            targets.len()
        );

        Ok(())
    }

    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        debug!("ModelManager: delete_model called for: {}", model_id);

        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        debug!("ModelManager: Found model info: {:?}", model_info);

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));
        debug!("ModelManager: Model path: {:?}", model_path);
        debug!("ModelManager: Partial path: {:?}", partial_path);

        let mut deleted_something = false;

        if model_info.is_directory {
            // Delete complete model directory if it exists
            if model_path.exists() && model_path.is_dir() {
                info!("Deleting model directory at: {:?}", model_path);
                fs::remove_dir_all(&model_path)?;
                info!("Model directory deleted successfully");
                deleted_something = true;
            }
        } else {
            // Delete complete model file if it exists
            if model_path.exists() {
                info!("Deleting model file at: {:?}", model_path);
                fs::remove_file(&model_path)?;
                info!("Model file deleted successfully");
                deleted_something = true;
            }
        }

        // Delete partial file if it exists (same for both types)
        if partial_path.exists() {
            info!("Deleting partial file at: {:?}", partial_path);
            fs::remove_file(&partial_path)?;
            info!("Partial file deleted successfully");
            deleted_something = true;
        }

        if !deleted_something {
            return Err(anyhow::anyhow!("No model files found to delete"));
        }

        // Custom models should be removed from the list entirely since they
        // have no download URL and can't be re-downloaded
        if model_info.is_custom {
            let mut models = self.available_models.lock().unwrap();
            models.remove(model_id);
            debug!("ModelManager: removed custom model from available models");
        } else {
            // Update download status (marks predefined models as not downloaded)
            self.update_download_status()?;
            debug!("ModelManager: download status updated");
        }

        // Emit event to notify UI
        self.events.emit(
            "model-deleted",
            serde_json::to_value(model_id).unwrap_or_default(),
        );

        Ok(())
    }

    pub fn get_model_path(&self, model_id: &str) -> Result<PathBuf> {
        let model_info = self
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            return Err(anyhow::anyhow!("Model not available: {}", model_id));
        }

        // Ensure we don't return partial files/directories
        if model_info.is_downloading {
            return Err(anyhow::anyhow!(
                "Model is currently downloading: {}",
                model_id
            ));
        }

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        if !model_info.files.is_empty() {
            // Multi-file layout: complete means every pinned file is present.
            return if multi_file_complete(&self.models_dir, &model_info) {
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model directory not found: {}",
                    model_id
                ))
            };
        }

        if model_info.is_directory {
            // For directory-based models, ensure the directory exists and is complete
            if model_path.exists() && model_path.is_dir() && !partial_path.exists() {
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model directory not found: {}",
                    model_id
                ))
            }
        } else {
            // For file-based models (existing logic)
            if model_path.exists() && !partial_path.exists() {
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model file not found: {}",
                    model_id
                ))
            }
        }
    }

    /// Returns true exactly once per flag lifetime (first caller runs the ensure).
    #[cfg_attr(not(feature = "diarization"), allow(dead_code))]
    fn should_run_diarization_ensure(flag: &std::sync::atomic::AtomicBool) -> bool {
        flag.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_ok()
    }

    /// Ensures the diarization models are present (either in the local models/diarization
    /// directory or in the hf-hub cache). Downloads from HuggingFace if missing.
    #[cfg(feature = "diarization")]
    pub fn ensure_diarization_models(&self) -> Result<()> {
        if !Self::should_run_diarization_ensure(&self.diarization_ensured) {
            return Ok(());
        }

        info!("Ensuring diarization models via speakrs (once per session)...");
        self.events.emit(
            "model-setup-started",
            serde_json::to_value("diarization").unwrap_or_default(),
        );

        use speakrs::{ExecutionMode, OwnedDiarizationPipeline};
        if let Err(e) = OwnedDiarizationPipeline::from_pretrained(ExecutionMode::Cpu) {
            // allow a retry next time if the one-shot failed
            self.diarization_ensured
                .store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(anyhow::anyhow!(
                "Failed to download/load diarization models: {}",
                e
            ));
        }

        self.events.emit(
            "model-setup-completed",
            serde_json::to_value("diarization").unwrap_or_default(),
        );
        Ok(())
    }

    #[cfg(not(feature = "diarization"))]
    pub fn ensure_diarization_models(&self) -> Result<()> {
        anyhow::bail!(
            "diarization not included in this build (rebuild with --features diarization)"
        );
    }

    pub fn cancel_download(&self, model_id: &str) -> Result<()> {
        debug!("ModelManager: cancel_download called for: {}", model_id);

        // Set the cancellation flag to stop the download loop
        {
            let flags = self.cancel_flags.lock().unwrap();
            if let Some(flag) = flags.get(model_id) {
                flag.store(true, Ordering::Relaxed);
                info!("Cancellation flag set for: {}", model_id);
            } else {
                warn!("No active download found for: {}", model_id);
            }
        }

        // Update state immediately for UI responsiveness
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
            }
        }

        // Update download status to reflect current state
        self.update_download_status()?;

        // Emit cancellation event so all UI components can clear their state
        self.events.emit(
            "model-download-cancelled",
            serde_json::to_value(model_id).unwrap_or_default(),
        );

        info!("Download cancellation initiated for: {}", model_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_discover_custom_whisper_models() {
        let temp_dir = TempDir::new().unwrap();
        let models_dir = temp_dir.path().to_path_buf();

        // Create test .bin files
        let mut custom_file = File::create(models_dir.join("my-custom-model.bin")).unwrap();
        custom_file.write_all(b"fake model data").unwrap();

        let mut another_file = File::create(models_dir.join("whisper_medical_v2.bin")).unwrap();
        another_file.write_all(b"another fake model").unwrap();

        // Create files that should be ignored
        File::create(models_dir.join(".hidden-model.bin")).unwrap(); // Hidden file
        File::create(models_dir.join("readme.txt")).unwrap(); // Non-.bin file
        File::create(models_dir.join("ggml-small.bin")).unwrap(); // Predefined filename
        fs::create_dir(models_dir.join("some-directory.bin")).unwrap(); // Directory

        // Set up available_models with a predefined Whisper model
        let mut models = HashMap::new();
        models.insert(
            "small".to_string(),
            ModelInfo {
                id: "small".to_string(),
                name: "Whisper Small".to_string(),
                description: "Test".to_string(),
                filename: "ggml-small.bin".to_string(),
                url: Some("https://example.com".to_string()),
                sha256: None,
                size_mb: 100,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.5,
                speed_score: 0.5,
                supports_translation: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                supports_language_selection: true,
                is_custom: false,
                files: vec![],
            },
        );

        // Discover custom models
        ModelManager::discover_custom_whisper_models(&models_dir, &mut models).unwrap();

        // Should have discovered 2 custom models (my-custom-model and whisper_medical_v2)
        assert!(models.contains_key("my-custom-model"));
        assert!(models.contains_key("whisper_medical_v2"));

        // Verify custom model properties
        let custom = models.get("my-custom-model").unwrap();
        assert_eq!(custom.name, "My Custom Model");
        assert_eq!(custom.filename, "my-custom-model.bin");
        assert!(custom.url.is_none()); // Custom models have no URL
        assert!(custom.is_downloaded);
        assert!(custom.is_custom);
        assert_eq!(custom.accuracy_score, 0.0);
        assert_eq!(custom.speed_score, 0.0);
        assert!(custom.supported_languages.is_empty());

        // Verify underscore handling
        let medical = models.get("whisper_medical_v2").unwrap();
        assert_eq!(medical.name, "Whisper Medical V2");

        // Should NOT have discovered hidden, non-.bin, predefined, or directories
        assert!(!models.contains_key(".hidden-model"));
        assert!(!models.contains_key("readme"));
        assert!(!models.contains_key("some-directory"));
    }

    #[test]
    fn test_discover_custom_models_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let models_dir = temp_dir.path().to_path_buf();

        let mut models = HashMap::new();
        let count_before = models.len();

        ModelManager::discover_custom_whisper_models(&models_dir, &mut models).unwrap();

        // No new models should be added
        assert_eq!(models.len(), count_before);
    }

    #[test]
    fn test_discover_custom_models_nonexistent_dir() {
        let models_dir = PathBuf::from("/nonexistent/path/that/does/not/exist");

        let mut models = HashMap::new();
        let count_before = models.len();

        // Should not error, just return Ok
        let result = ModelManager::discover_custom_whisper_models(&models_dir, &mut models);
        assert!(result.is_ok());
        assert_eq!(models.len(), count_before);
    }

    // ── SHA256 verification tests ─────────────────────────────────────────────

    /// Helper: write `data` to a temp file and return (TempDir, path).
    /// TempDir must be kept alive for the duration of the test.
    fn write_temp_file(data: &[u8]) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("model.partial");
        let mut f = File::create(&path).unwrap();
        f.write_all(data).unwrap();
        (dir, path)
    }

    #[test]
    fn test_verify_sha256_skipped_when_none() {
        // Custom models have no expected hash — verification must be a no-op.
        let (_dir, path) = write_temp_file(b"anything");
        assert!(ModelManager::verify_sha256(&path, None, "custom").is_ok());
        assert!(
            path.exists(),
            "file must be untouched when verification is skipped"
        );
    }

    #[test]
    fn test_verify_sha256_passes_on_correct_hash() {
        // Compute the real hash so the test is self-consistent.
        let (_dir, path) = write_temp_file(b"hello world");
        let actual = ModelManager::compute_sha256(&path).unwrap();
        assert!(
            ModelManager::verify_sha256(&path, Some(&actual), "test_model").is_ok(),
            "should pass when hash matches"
        );
        assert!(
            path.exists(),
            "file must be kept on successful verification"
        );
    }

    #[test]
    fn test_verify_sha256_fails_and_deletes_partial_on_mismatch() {
        let (_dir, path) = write_temp_file(b"this is not the real model");
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = ModelManager::verify_sha256(&path, Some(wrong_hash), "bad_model");

        assert!(result.is_err(), "mismatch must return an error");
        assert!(
            result.unwrap_err().to_string().contains("corrupt"),
            "error message should mention corruption"
        );
        assert!(
            !path.exists(),
            "partial file must be deleted after hash mismatch"
        );
    }

    #[test]
    fn test_verify_sha256_fails_and_deletes_partial_when_file_missing() {
        // Simulate a partial file that was already removed (e.g. disk full mid-download).
        let dir = TempDir::new().unwrap();
        let missing_path = dir.path().join("gone.partial");
        // Don't create the file — it should not exist.

        let result =
            ModelManager::verify_sha256(&missing_path, Some("anyexpectedhash"), "missing_model");

        assert!(result.is_err(), "missing file must return an error");
    }

    #[test]
    fn diarization_ensure_is_once() {
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // first call: not yet ensured -> should run (compare_exchange succeeds)
        assert!(ModelManager::should_run_diarization_ensure(&flag));
        // second call: already ensured -> should skip
        assert!(!ModelManager::should_run_diarization_ensure(&flag));
    }

    // ── Multi-file model helpers ──────────────────────────────────────────────

    #[test]
    fn multi_file_target_joins_under_model_dir() {
        let t = multi_file_target(Path::new("C:/m"), "supertonic-3", "onnx/vocoder.onnx").unwrap();
        assert!(t.ends_with(Path::new("supertonic-3/onnx/vocoder.onnx")));
    }

    #[test]
    fn multi_file_target_rejects_traversal() {
        assert!(multi_file_target(Path::new("C:/m"), "supertonic-3", "../evil.dll").is_err());
        assert!(multi_file_target(Path::new("C:/m"), "supertonic-3", "a/../../evil").is_err());
        assert!(multi_file_target(Path::new("C:/m"), "supertonic-3", "/abs/path").is_err());
        // Drive-letter prefixes only parse as Component::Prefix on Windows;
        // on POSIX "C:/abs" is a plain relative path, so gate this assert.
        #[cfg(windows)]
        assert!(multi_file_target(Path::new("C:/m"), "supertonic-3", "C:/abs").is_err());
        assert!(multi_file_target(Path::new("C:/m"), "supertonic-3", "").is_err());
    }

    /// Helper: a multi-file ModelInfo fixture with two pinned files.
    fn multi_file_info(filename: &str) -> ModelInfo {
        ModelInfo {
            id: "svt-test".to_string(),
            name: "Supertonic Test".to_string(),
            description: "multi-file completeness fixture".to_string(),
            filename: filename.to_string(),
            url: Some("https://example.invalid/supertonic".to_string()),
            sha256: None,
            size_mb: 1,
            is_downloaded: false,
            is_downloading: false,
            partial_size: 0,
            is_directory: true,
            engine_type: EngineType::SupertonicTts,
            accuracy_score: 0.0,
            speed_score: 0.0,
            supports_translation: false,
            is_recommended: false,
            supported_languages: vec!["en".to_string(), "ru".to_string()],
            supports_language_selection: false,
            is_custom: false,
            files: vec![
                ModelFileEntry {
                    path: "onnx/a.bin".to_string(),
                    url: "https://example.invalid/onnx/a.bin".to_string(),
                    sha256: "0".repeat(64),
                },
                ModelFileEntry {
                    path: "b.json".to_string(),
                    url: "https://example.invalid/b.json".to_string(),
                    sha256: "1".repeat(64),
                },
            ],
        }
    }

    #[test]
    fn multi_file_complete_requires_every_file() {
        let dir = std::env::temp_dir().join(format!("svt-mm-{}", std::process::id()));
        let model_dir = dir.join("m");
        std::fs::create_dir_all(model_dir.join("onnx")).unwrap();

        let mut info = multi_file_info("not-downloaded-yet");
        assert!(
            !multi_file_complete(&dir, &info),
            "no files on disk -> incomplete"
        );

        std::fs::write(model_dir.join("onnx/a.bin"), b"x").unwrap();
        assert!(
            !multi_file_complete(&dir, &info),
            "filename points elsewhere -> still incomplete"
        );

        info.filename = "m".into();
        assert!(
            !multi_file_complete(&dir, &info),
            "one of two files present -> incomplete"
        );

        std::fs::write(model_dir.join("b.json"), b"y").unwrap();
        assert!(
            multi_file_complete(&dir, &info),
            "all files present -> complete"
        );

        std::fs::remove_file(model_dir.join("b.json")).unwrap();
        assert!(
            !multi_file_complete(&dir, &info),
            "a removed file flips completeness back to false"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multi_file_complete_rejects_unsafe_entries() {
        let dir = std::env::temp_dir().join(format!("svt-mm-evil-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("m")).unwrap();

        let mut info = multi_file_info("m");
        info.files[0].path = "../evil.dll".to_string();
        // Even if the traversal target existed, an unsafe entry must never
        // count as "complete".
        assert!(!multi_file_complete(&dir, &info));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
