//! Whole-WAV local readback via the router's local (Piper) tier, with a
//! synthetic language-only profile (no cloned voice profile needed).
use echo_voice::router::VoiceRouter;
use echo_voice::{LanguageHint, SynthOpts, VoiceProfile};

pub struct LocalReadbackEngine {
    router: VoiceRouter,
    default_language: LanguageHint,
}

impl LocalReadbackEngine {
    pub fn new(router: VoiceRouter, default_language: LanguageHint) -> Self {
        Self {
            router,
            default_language,
        }
    }

    /// Synthesize the whole utterance via the router (local tier). Uses a
    /// synthetic profile carrying only the language hint — the local engine
    /// ignores profile refs.
    pub fn synthesize_whole_wav(&self, text: &str, rate: f32) -> Result<Vec<u8>, String> {
        let profile = VoiceProfile {
            id: "local".to_string(),
            display_name: "Local".to_string(),
            language_hint: self.default_language,
            created: String::new(),
            notes: String::new(),
            refs: vec![],
        };
        self.router
            .synthesize(text, &profile, &SynthOpts { rate })
            .map(|(wav, _used)| wav)
            .map_err(|e| e.to_string())
    }
}
