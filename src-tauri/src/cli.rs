use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "echo", about = "Echo - Speech to Text")]
pub struct CliArgs {
    /// Start with the main window hidden
    #[arg(long)]
    pub start_hidden: bool,

    /// Disable the system tray icon
    #[arg(long)]
    pub no_tray: bool,

    /// Toggle transcription on/off (sent to running instance)
    #[arg(long)]
    pub toggle_transcription: bool,

    /// Toggle transcription with post-processing on/off (sent to running instance)
    #[arg(long)]
    pub toggle_post_process: bool,

    /// Cancel the current operation (sent to running instance)
    #[arg(long)]
    pub cancel: bool,

    /// Enable debug mode with verbose logging
    #[arg(long)]
    pub debug: bool,

    /// Path to an audio/video file for offline transcription
    #[arg(long, value_name = "FILE")]
    pub transcribe_file: Option<PathBuf>,

    /// Path to save the resulting transcript (plain text). Prints to stdout if omitted.
    #[arg(short, long, value_name = "OUTPUT")]
    pub output: Option<PathBuf>,

    /// Language for offline transcription: "auto" (default) or a code like "ru"/"en".
    /// "auto" enables RU/EN code-switch steering on Whisper models.
    #[arg(long, value_name = "LANG")]
    pub language: Option<String>,

    /// Model id to use for offline transcription (e.g. "turbo"). Defaults to the
    /// app's selected model. Use a Whisper model for best RU/EN mixing.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Output format for offline transcription: plain (default), inline, srt, vtt, json, karaoke, speaker.
    /// `speaker` groups diarized turns as `[Speaker N] (M:SS - M:SS)` blocks.
    #[arg(long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// Enable speaker diarization (label sections with Speaker 1/2/...).
    #[arg(long)]
    pub diarize: bool,

    /// Optional known speaker count hint for diarization (else auto-detected).
    #[arg(long, value_name = "N")]
    pub speakers: Option<usize>,

    /// Translate the transcript into a target language offline (Hy-MT) and emit the
    /// translated plain text. Value is a language code like "en"/"ru"/"uk".
    #[arg(long, value_name = "LANG")]
    pub translate: Option<String>,

    /// Ask the user a question via the running Echo instance and print the answer
    #[arg(long, value_name = "TEXT")]
    pub ask: Option<String>,
    /// Comma-separated options (makes --ask a choice question)
    #[arg(long, value_name = "A,B,C")]
    pub ask_options: Option<String>,
    /// Timeout in seconds for --ask (default 300)
    #[arg(long, default_value_t = 300)]
    pub ask_timeout: u64,
    /// Speak the question aloud (TTS)
    #[arg(long, default_value_t = false)]
    pub ask_speak: bool,
    /// Agent Bridge port (default 4123)
    #[arg(long, default_value_t = 4123)]
    pub ask_port: u16,

    /// Create a voice profile from a reference clip: --voice-add "Name" --ref clip.wav
    #[arg(long, value_name = "NAME")]
    pub voice_add: Option<String>,
    /// Reference audio for --voice-add (repeatable)
    #[arg(long = "ref", value_name = "WAV")]
    pub voice_ref: Vec<PathBuf>,
    /// Exact transcript for the (single) --ref; auto-transcribed via Echo STT if omitted
    #[arg(long, value_name = "TEXT")]
    pub ref_text: Option<String>,
    /// Language hint for --voice-add: ru | en | mixed (default mixed)
    #[arg(long, value_name = "LANG")]
    pub voice_lang: Option<String>,
    /// List voice profiles and exit
    #[arg(long)]
    pub voice_list: bool,
    /// Remove a voice profile by id and exit
    #[arg(long, value_name = "ID")]
    pub voice_remove: Option<String>,
    /// Narrate a markdown/text file to a WAV using a cloned voice profile
    #[arg(long, value_name = "FILE")]
    pub narrate: Option<PathBuf>,
    /// Voice profile id for --narrate (and default for readback CLI)
    #[arg(long, value_name = "ID")]
    pub voice: Option<String>,
    /// Tier override for voice ops: auto | local | sidecar
    #[arg(long, value_name = "TIER")]
    pub tier: Option<String>,
}
