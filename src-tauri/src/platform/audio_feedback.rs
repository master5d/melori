use crate::settings::SoundTheme;
use crate::settings::{self, AppSettings};
use cpal::traits::{DeviceTrait, HostTrait};
use log::{debug, error, warn};
use rodio::OutputStreamBuilder;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::thread;
use tauri::{AppHandle, Manager};

pub enum SoundType {
    Start,
    Stop,
}

fn resolve_sound_path(
    app: &AppHandle,
    settings: &AppSettings,
    sound_type: SoundType,
) -> Option<PathBuf> {
    let sound_file = get_sound_path(settings, sound_type);
    let base_dir = get_sound_base_dir(settings);
    match base_dir {
        tauri::path::BaseDirectory::AppData => {
            crate::portable::resolve_app_data(app, &sound_file).ok()
        }
        _ => app.path().resolve(&sound_file, base_dir).ok(),
    }
}

fn get_sound_path(settings: &AppSettings, sound_type: SoundType) -> String {
    match (settings.sound_theme, sound_type) {
        (SoundTheme::Custom, SoundType::Start) => "custom_start.wav".to_string(),
        (SoundTheme::Custom, SoundType::Stop) => "custom_stop.wav".to_string(),
        (_, SoundType::Start) => settings.sound_theme.to_start_path(),
        (_, SoundType::Stop) => settings.sound_theme.to_stop_path(),
    }
}

fn get_sound_base_dir(settings: &AppSettings) -> tauri::path::BaseDirectory {
    match settings.sound_theme {
        SoundTheme::Custom => tauri::path::BaseDirectory::AppData,
        _ => tauri::path::BaseDirectory::Resource,
    }
}

pub fn play_feedback_sound(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if !settings.audio_feedback {
        return;
    }
    if let Some(path) = resolve_sound_path(app, &settings, sound_type) {
        play_sound_async(app, path);
    }
}

pub fn play_feedback_sound_blocking(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if !settings.audio_feedback {
        return;
    }
    if let Some(path) = resolve_sound_path(app, &settings, sound_type) {
        play_sound_blocking(app, &path);
    }
}

pub fn play_test_sound(app: &AppHandle, sound_type: SoundType) {
    let settings = settings::get_settings(app);
    if let Some(path) = resolve_sound_path(app, &settings, sound_type) {
        play_sound_blocking(app, &path);
    }
}

fn play_sound_async(app: &AppHandle, path: PathBuf) {
    let app_handle = app.clone();
    thread::spawn(move || {
        if let Err(e) = play_sound_at_path(&app_handle, path.as_path()) {
            error!("Failed to play sound '{}': {}", path.display(), e);
        }
    });
}

fn play_sound_blocking(app: &AppHandle, path: &Path) {
    if let Err(e) = play_sound_at_path(app, path) {
        error!("Failed to play sound '{}': {}", path.display(), e);
    }
}

fn play_sound_at_path(app: &AppHandle, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let settings = settings::get_settings(app);
    let volume = settings.audio_feedback_volume;
    let selected_device = settings.selected_output_device.clone();
    play_audio_file(path, selected_device, volume)
}

/// Resolve a selected output-device name (or `None`/"Default") to an opened
/// rodio output stream. Shared by feedback-sound playback and the Audio-console
/// monitor thread.
///
/// `on_error` receives cpal stream errors raised AFTER the stream is running —
/// most importantly a mid-session device disconnect, which the open-time `Result`
/// structurally cannot see. Feedback sounds pass `None` (fire-and-forget); the
/// monitor thread passes a callback that stops itself and emits `monitor-error`.
pub(crate) fn resolve_output_stream(
    selected_device: Option<String>,
    on_error: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
) -> Result<rodio::OutputStream, Box<dyn std::error::Error>> {
    // Resolve the device FIRST, so the builder is constructed exactly once and
    // `with_error_callback` (which rewrites the builder's type parameter) is
    // applied exactly once. Attaching it per match-arm would both move the
    // closure twice and give the arms mismatched types.
    let device: Option<cpal::Device> = match selected_device {
        Some(device_name) if device_name != "Default" => {
            let host = crate::audio_toolkit::get_cpal_host();
            let mut found = None;
            for device in host.output_devices()? {
                if device.name()? == device_name {
                    found = Some(device);
                    break;
                }
            }
            if found.is_none() {
                warn!("Device '{}' not found, using default device", device_name);
            }
            found
        }
        _ => {
            debug!("Using default device");
            None
        }
    };

    let builder = match device {
        Some(device) => OutputStreamBuilder::from_device(device)?,
        None => OutputStreamBuilder::from_default_device()?,
    };

    Ok(builder
        .with_error_callback(move |err: cpal::StreamError| {
            let msg = format!("Output stream error: {err}");
            error!("{msg}");
            if let Some(cb) = &on_error {
                cb(msg);
            }
        })
        .open_stream()?)
}

fn play_audio_file(
    path: &std::path::Path,
    selected_device: Option<String>,
    volume: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream_handle = resolve_output_stream(selected_device, None)?;
    let mixer = stream_handle.mixer();

    let file = File::open(path)?;
    let buf_reader = BufReader::new(file);

    let sink = rodio::play(mixer, buf_reader)?;
    sink.set_volume(volume);
    sink.sleep_until_end();

    Ok(())
}
