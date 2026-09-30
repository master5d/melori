use crate::tts::{TtsManager, VoiceInfo};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub fn tts_list_voices(tts_manager: State<Arc<TtsManager>>) -> Result<Vec<VoiceInfo>, String> {
    tts_manager.list_voices()
}

#[tauri::command]
#[specta::specta]
pub fn tts_speak(
    tts_manager: State<Arc<TtsManager>>,
    text: String,
    voice_id: Option<String>,
    rate: Option<f32>,
) -> Result<(), String> {
    tts_manager.speak(text, voice_id, rate.unwrap_or(1.0))
}

#[tauri::command]
#[specta::specta]
pub fn tts_stop(tts_manager: State<Arc<TtsManager>>) -> Result<(), String> {
    tts_manager.stop()
}
