// Re-export all audio components
mod device;
mod gain;
mod health;
mod meter;
mod monitor;
mod recorder;
mod resampler;
mod spectrum;
mod utils;
mod visualizer;

pub use device::{list_input_devices, list_output_devices, CpalDeviceInfo};
pub use gain::db_to_linear;
pub use health::{
    DeviceFacts, DeviceHealthState, HealthEvaluator, HealthSnapshot, HealthStatus, SILENCE_DBFS,
    SILENCE_MS, STALL_MS,
};
pub use monitor::{apply_monitor, MonitorBuf, MonitorRamp};
pub use recorder::{is_microphone_access_denied, is_no_input_device_error, AudioRecorder};
pub use resampler::FrameResampler;
pub use spectrum::{band_centers, SpectrumAnalyzer};
pub use utils::{read_wav_samples, save_wav_file, verify_wav_file};
pub use visualizer::AudioVisualiser;
