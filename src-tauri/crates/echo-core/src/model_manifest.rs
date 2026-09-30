use crate::model::{EngineType, ModelFileEntry, ModelInfo};
use anyhow::Result;
use serde::Deserialize;

/// Embedded model registry. Source of truth for download URLs, checksums, and
/// metadata. Provenance fields live here only (not in the specta-exported
/// `ModelInfo`). Populated by `scripts/sovereign-models.mjs`.
pub const MODELS_MANIFEST_JSON: &str = include_str!("../../../resources/models.json");

#[derive(Debug, Clone, Deserialize)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub models: Vec<ModelManifestEntry>,
}

// Provenance fields (provenance_class/upstream_repo/license/layout/mirrored/
// converted) are consumed out-of-band — by serde validation here and by
// scripts/sovereignty-gate.mjs — not by Rust, hence allow(dead_code).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ModelManifestEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub mirror_path: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    pub size_mb: u64,
    pub is_directory: bool,
    pub engine_type: EngineType,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub supports_translation: bool,
    pub is_recommended: bool,
    pub supported_languages: Vec<String>,
    pub supports_language_selection: bool,
    // ── provenance (manifest-only) ──────────────────────────────────────────
    pub provenance_class: u8,
    pub upstream_repo: String,
    pub license: String,
    pub layout: String,
    /// True for artifacts with no official source, mirrored byte-for-byte to own R2.
    #[serde(default)]
    pub mirrored: bool,
    /// True for artifacts with no official source, self-converted to ONNX int8 and hosted on own R2.
    #[serde(default)]
    pub converted: bool,
    /// Per-file (path/url/sha256) pins for multi-file layouts (e.g. supertonic-3).
    #[serde(default)]
    pub files: Option<Vec<ModelFileEntry>>,
}

impl ModelManifestEntry {
    pub fn to_model_info(&self) -> ModelInfo {
        ModelInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            filename: self.filename.clone(),
            url: self.url.clone(),
            sha256: self.sha256.clone(),
            size_mb: self.size_mb,
            is_downloaded: false,
            is_downloading: false,
            partial_size: 0,
            is_directory: self.is_directory,
            engine_type: self.engine_type.clone(),
            accuracy_score: self.accuracy_score,
            speed_score: self.speed_score,
            supports_translation: self.supports_translation,
            is_recommended: self.is_recommended,
            supported_languages: self.supported_languages.clone(),
            supports_language_selection: self.supports_language_selection,
            is_custom: false,
            files: self.files.clone().unwrap_or_default(),
        }
    }
}

/// Resolve a manifest entry without ever falling back to an undisclosed mirror.
/// Official URLs always win; mirror-only entries require an explicit setting.
pub fn resolve_download_url(entry: &ModelManifestEntry, mirror_base: &str) -> Option<String> {
    if let Some(url) = entry.url.as_deref().filter(|url| !url.is_empty()) {
        return Some(url.to_string());
    }
    let path = entry.mirror_path.as_deref()?.trim_start_matches('/');
    let base = mirror_base.trim_end_matches('/');
    if base.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{}/{}", base, path))
}

pub fn load_manifest() -> Result<ModelManifest> {
    let manifest: ModelManifest = serde_json::from_str(MODELS_MANIFEST_JSON)?;
    anyhow::ensure!(
        manifest.schema_version == 1,
        "unsupported models.json schema_version {} (expected 1)",
        manifest.schema_version
    );
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "schema_version": 1,
      "models": [
        {
          "id": "small",
          "name": "Whisper Small",
          "description": "Fast and fairly accurate.",
          "filename": "ggml-small.bin",
          "url": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
          "sha256": "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
          "size_mb": 465,
          "is_directory": false,
          "engine_type": "Whisper",
          "accuracy_score": 0.60,
          "speed_score": 0.85,
          "supports_translation": true,
          "is_recommended": false,
          "supported_languages": ["en", "ru"],
          "supports_language_selection": true,
          "provenance_class": 1,
          "upstream_repo": "ggerganov/whisper.cpp",
          "license": "MIT",
          "layout": "file"
        }
      ]
    }"#;

    #[test]
    fn parses_fixture_and_maps_to_model_info() {
        let m: ModelManifest = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(m.schema_version, 1);
        assert_eq!(m.models.len(), 1);
        let info = m.models[0].to_model_info();
        assert_eq!(info.id, "small");
        assert_eq!(
            info.url.as_deref(),
            Some("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin")
        );
        assert_eq!(
            info.sha256.as_deref(),
            Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b")
        );
        assert!(!info.is_custom);
        assert!(!info.is_downloaded);
        assert!(matches!(info.engine_type, EngineType::Whisper));
    }

    #[test]
    fn sha256_defaults_to_none_when_absent() {
        let json = FIXTURE.replace(
            "\"sha256\": \"1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b\",",
            "",
        );
        let m: ModelManifest = serde_json::from_str(&json).unwrap();
        assert!(m.models[0].sha256.is_none());
    }

    #[test]
    fn embedded_manifest_is_valid() {
        let m = load_manifest().expect("embedded models.json must parse");
        assert!(
            m.models.len() >= 16,
            "expected all predefined models, got {}",
            m.models.len()
        );
        let mut ids = std::collections::HashSet::new();
        for e in &m.models {
            assert!(ids.insert(e.id.clone()), "duplicate model id: {}", e.id);
            assert!(
                e.url.as_deref().is_some_and(|url| !url.is_empty()) || e.mirror_path.is_some(),
                "model {} needs an official url or mirror_path",
                e.id
            );
            assert!(
                !e.url
                    .as_deref()
                    .unwrap_or_default()
                    .contains("blob.handy.computer"),
                "model {} still points at Handy CDN",
                e.id
            );
            assert!(
                (1..=3).contains(&e.provenance_class),
                "model {} has invalid provenance_class {}",
                e.id,
                e.provenance_class
            );
        }
        let recommended = m.models.iter().filter(|e| e.is_recommended).count();
        assert_eq!(recommended, 1, "exactly one model must be recommended");
    }

    fn mirror_entry(path: Option<&str>, url: Option<&str>) -> ModelManifestEntry {
        let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let entry = value["models"][0].as_object_mut().unwrap();
        entry.insert("url".into(), serde_json::json!(url));
        entry.insert("mirror_path".into(), serde_json::json!(path));
        serde_json::from_value(value["models"][0].clone()).unwrap()
    }

    #[test]
    fn mirror_entry_without_setting_is_unavailable() {
        assert_eq!(
            resolve_download_url(&mirror_entry(Some("canary.tar.gz"), None), ""),
            None
        );
    }

    #[test]
    fn mirror_entry_uses_configured_base() {
        assert_eq!(
            resolve_download_url(
                &mirror_entry(Some("canary.tar.gz"), None),
                "https://mirror.example/models/"
            )
            .as_deref(),
            Some("https://mirror.example/models/canary.tar.gz")
        );
    }

    #[test]
    fn official_entry_ignores_mirror() {
        assert_eq!(
            resolve_download_url(
                &mirror_entry(None, Some("https://huggingface.co/x.bin")),
                "https://mirror.example"
            )
            .as_deref(),
            Some("https://huggingface.co/x.bin")
        );
    }

    #[test]
    fn multi_file_entry_parses_and_maps() {
        let json = r#"{
          "schema_version": 1,
          "models": [{
            "id": "supertonic-3", "name": "Supertonic 3", "description": "d",
            "filename": "supertonic-3",
            "url": "https://huggingface.co/Supertone/supertonic-3",
            "size_mb": 384, "is_directory": true, "engine_type": "SupertonicTts",
            "accuracy_score": 0.0, "speed_score": 0.0,
            "supports_translation": false, "is_recommended": false,
            "supported_languages": ["en","ru"], "supports_language_selection": false,
            "provenance_class": 1, "upstream_repo": "Supertone/supertonic-3",
            "license": "OpenRAIL-M", "layout": "multi-file",
            "files": [
              {"path": "onnx/duration_predictor.onnx",
               "url": "https://huggingface.co/Supertone/supertonic-3/resolve/main/onnx/duration_predictor.onnx",
               "sha256": "c3eb91414d5ff8a7a239b7fe9e34e7e2bf8a8140d8375ffb14718b1c639325db"}
            ]
          }]
        }"#;
        let m: ModelManifest = serde_json::from_str(json).unwrap();
        let info = m.models[0].to_model_info();
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.files[0].path, "onnx/duration_predictor.onnx");
        assert!(matches!(info.engine_type, EngineType::SupertonicTts));
    }

    #[test]
    fn single_file_entries_have_empty_files() {
        let m: ModelManifest = serde_json::from_str(FIXTURE).unwrap();
        assert!(m.models[0].to_model_info().files.is_empty());
    }

    #[test]
    fn supertonic_is_not_asr() {
        assert!(!EngineType::SupertonicTts.is_asr());
        assert!(!EngineType::PiperTts.is_asr());
        assert!(EngineType::Whisper.is_asr());
    }

    #[test]
    fn embedded_manifest_has_supertonic_with_17_pinned_files() {
        let m = load_manifest().unwrap();
        let e = m
            .models
            .iter()
            .find(|e| e.id == "supertonic-3")
            .expect("supertonic-3 entry");
        let files = e.files.as_ref().expect("files list");
        assert_eq!(files.len(), 17);
        assert!(files.iter().all(|f| f.sha256.len() == 64));
        assert!(files.iter().all(|f| f
            .url
            .starts_with("https://huggingface.co/Supertone/supertonic-3/resolve/main/")));
        assert!(files.iter().all(|f| !f.path.contains("..")));
    }
}
