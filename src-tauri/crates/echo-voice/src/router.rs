use crate::{SynthOpts, VoiceEngine, VoiceError, VoiceProfile, VoiceTier};

pub struct VoiceRouter {
    tier: VoiceTier,
    cloud: Option<Box<dyn VoiceEngine>>,
    local: Option<Box<dyn VoiceEngine>>,
    sidecar: Option<Box<dyn VoiceEngine>>,
    sidecar_health: Box<dyn Fn() -> bool + Send + Sync>,
}

impl VoiceRouter {
    pub fn new(
        tier: VoiceTier,
        cloud: Option<Box<dyn VoiceEngine>>,
        local: Option<Box<dyn VoiceEngine>>,
        sidecar: Option<Box<dyn VoiceEngine>>,
        sidecar_health: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            tier,
            cloud,
            local,
            sidecar,
            sidecar_health,
        }
    }

    /// Whether the sidecar tier is currently reachable. Used by the host to
    /// decide cloned-stream vs SAPI-whole before starting an utterance.
    pub fn health(&self) -> bool {
        (self.sidecar_health)()
    }

    /// Whether this router's tier permits the sidecar tier (used by the readback
    /// consumer to decide streaming eligibility).
    pub fn allows_sidecar(&self) -> bool {
        matches!(self.tier, VoiceTier::Auto | VoiceTier::SidecarOnly)
    }

    pub fn has_cloud(&self) -> bool {
        self.cloud.is_some()
    }

    pub fn synthesize(
        &self,
        text: &str,
        profile: &VoiceProfile,
        opts: &SynthOpts,
    ) -> Result<(Vec<u8>, &'static str), VoiceError> {
        // Build the ordered candidate list per tier.
        let order: Vec<&Option<Box<dyn VoiceEngine>>> = match self.tier {
            VoiceTier::SidecarOnly => vec![&self.sidecar],
            VoiceTier::LocalOnly => vec![&self.local],
            VoiceTier::Auto => {
                if (self.sidecar_health)() {
                    vec![&self.cloud, &self.sidecar, &self.local]
                } else {
                    vec![&self.cloud, &self.local, &self.sidecar]
                }
            }
        };

        let mut last_err: Option<VoiceError> = None;
        for slot in order {
            if let Some(engine) = slot {
                match engine.synthesize(text, profile, opts) {
                    std::result::Result::Ok(bytes) => {
                        return std::result::Result::Ok((bytes, engine.id()))
                    }
                    std::result::Result::Err(e) => {
                        log::warn!("voice engine {} failed: {e}", engine.id());
                        last_err = Some(e);
                    }
                }
            }
        }
        std::result::Result::Err(last_err.unwrap_or(VoiceError::NoEngine))
    }

    /// Stream PCM from the sidecar tier when permitted and healthy. The whole reply
    /// is sent to the sidecar (server-side segmentation); no client chunking.
    pub fn synthesize_stream(
        &self,
        text: &str,
        profile: &VoiceProfile,
        opts: &SynthOpts,
        cancel: &std::sync::atomic::AtomicBool,
        on_pcm: &mut (dyn FnMut(&[i16]) + Send),
    ) -> Result<(), VoiceError> {
        if matches!(self.tier, VoiceTier::Auto) {
            if let Some(engine) = &self.cloud {
                match engine.synthesize_stream(text, profile, opts, cancel, on_pcm) {
                    std::result::Result::Ok(()) => return std::result::Result::Ok(()),
                    std::result::Result::Err(e) => {
                        log::warn!("cloud stream engine {} failed: {e}", engine.id());
                    }
                }
            }
        }
        let allow_sidecar = matches!(self.tier, VoiceTier::Auto | VoiceTier::SidecarOnly);
        if allow_sidecar {
            if let Some(engine) = &self.sidecar {
                if (self.sidecar_health)() {
                    return engine.synthesize_stream(text, profile, opts, cancel, on_pcm);
                }
            }
        }
        Err(VoiceError::NoEngine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LanguageHint;

    struct Ok(&'static str);
    impl VoiceEngine for Ok {
        fn id(&self) -> &'static str {
            self.0
        }
        fn synthesize(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            std::result::Result::Ok(b"RIFF".to_vec())
        }
    }
    struct Boom;
    impl VoiceEngine for Boom {
        fn id(&self) -> &'static str {
            "boom"
        }
        fn synthesize(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            std::result::Result::Err(VoiceError::Engine("boom".into()))
        }
    }
    fn prof() -> VoiceProfile {
        VoiceProfile {
            id: "p".into(),
            display_name: "P".into(),
            language_hint: LanguageHint::En,
            created: "n".into(),
            notes: String::new(),
            refs: vec![],
        }
    }

    #[test]
    fn auto_prefers_sidecar_when_healthy() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            None,
            Some(Box::new(Ok("local"))),
            Some(Box::new(Ok("sidecar_remote"))),
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "sidecar_remote");
    }

    #[test]
    fn auto_falls_to_local_when_sidecar_unhealthy() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            None,
            Some(Box::new(Ok("voxcpm_local"))),
            Some(Box::new(Ok("sidecar_remote"))),
            Box::new(|| false),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "voxcpm_local");
    }

    #[test]
    fn auto_falls_through_on_error() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            None,
            Some(Box::new(Ok("voxcpm_local"))),
            Some(Box::new(Boom)),
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "voxcpm_local");
    }

    #[test]
    fn auto_prefers_cloud_over_sidecar() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            Some(Box::new(Ok("fish_remote"))),
            Some(Box::new(Ok("local"))),
            Some(Box::new(Ok("sidecar_remote"))),
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "fish_remote");
    }

    #[test]
    fn cloud_error_falls_through_to_sidecar() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            Some(Box::new(Boom)),
            None,
            Some(Box::new(Ok("sidecar_remote"))),
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "sidecar_remote");
    }

    #[test]
    fn local_only_ignores_sidecar() {
        let r = VoiceRouter::new(
            VoiceTier::LocalOnly,
            None,
            Some(Box::new(Ok("voxcpm_local"))),
            Some(Box::new(Ok("sidecar_remote"))),
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "voxcpm_local");
    }

    #[test]
    fn local_only_ignores_cloud() {
        let r = VoiceRouter::new(
            VoiceTier::LocalOnly,
            Some(Box::new(Ok("fish_remote"))),
            Some(Box::new(Ok("voxcpm_local"))),
            None,
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "voxcpm_local");
    }

    #[test]
    fn sidecar_only_ignores_cloud() {
        let r = VoiceRouter::new(
            VoiceTier::SidecarOnly,
            Some(Box::new(Ok("fish_remote"))),
            None,
            Some(Box::new(Ok("sidecar_remote"))),
            Box::new(|| true),
        );
        let (_, used) = r.synthesize("x", &prof(), &SynthOpts::default()).unwrap();
        assert_eq!(used, "sidecar_remote");
    }

    #[test]
    fn health_reflects_probe_closure() {
        let up = VoiceRouter::new(VoiceTier::Auto, None, None, None, Box::new(|| true));
        let down = VoiceRouter::new(VoiceTier::Auto, None, None, None, Box::new(|| false));
        assert!(up.health());
        assert!(!down.health());
    }

    #[test]
    fn no_engine_errors() {
        let r = VoiceRouter::new(VoiceTier::Auto, None, None, None, Box::new(|| false));
        assert!(matches!(
            r.synthesize("x", &prof(), &SynthOpts::default()),
            Err(VoiceError::NoEngine)
        ));
    }

    struct PcmOk;
    impl VoiceEngine for PcmOk {
        fn id(&self) -> &'static str {
            "sidecar_remote"
        }
        fn synthesize(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            std::result::Result::Ok(b"RIFF".to_vec())
        }
        fn synthesize_stream(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
            _c: &std::sync::atomic::AtomicBool,
            on_pcm: &mut (dyn FnMut(&[i16]) + Send),
        ) -> Result<(), VoiceError> {
            on_pcm(&[42i16]);
            std::result::Result::Ok(())
        }
    }

    #[test]
    fn stream_routes_to_healthy_sidecar() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            None,
            None,
            Some(Box::new(PcmOk)),
            Box::new(|| true),
        );
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut got = Vec::new();
        r.synthesize_stream("x", &prof(), &SynthOpts::default(), &cancel, &mut |s| {
            got.extend_from_slice(s)
        })
        .unwrap();
        assert_eq!(got, vec![42i16]);
    }

    struct PcmBoom;
    impl VoiceEngine for PcmBoom {
        fn id(&self) -> &'static str {
            "fish_remote"
        }
        fn synthesize(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            std::result::Result::Ok(b"RIFF".to_vec())
        }
        fn synthesize_stream(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
            _c: &std::sync::atomic::AtomicBool,
            _on: &mut (dyn FnMut(&[i16]) + Send),
        ) -> Result<(), VoiceError> {
            std::result::Result::Err(VoiceError::Engine("cloud stream down".into()))
        }
    }

    #[test]
    fn stream_cloud_error_falls_back_to_sidecar_stream() {
        let r = VoiceRouter::new(
            VoiceTier::Auto,
            Some(Box::new(PcmBoom)),
            None,
            Some(Box::new(PcmOk)),
            Box::new(|| true),
        );
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut got = Vec::new();
        r.synthesize_stream("x", &prof(), &SynthOpts::default(), &cancel, &mut |s| {
            got.extend_from_slice(s)
        })
        .unwrap();
        assert_eq!(got, vec![42i16]); // дошли до PcmOk-сайдкара
    }

    #[test]
    fn has_cloud_reflects_slot() {
        let with = VoiceRouter::new(
            VoiceTier::Auto,
            Some(Box::new(Ok("fish_remote"))),
            None,
            None,
            Box::new(|| false),
        );
        let without = VoiceRouter::new(VoiceTier::Auto, None, None, None, Box::new(|| false));
        assert!(with.has_cloud());
        assert!(!without.has_cloud());
    }

    #[test]
    fn stream_errs_when_unhealthy_or_local_only() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let down = VoiceRouter::new(
            VoiceTier::Auto,
            None,
            None,
            Some(Box::new(PcmOk)),
            Box::new(|| false),
        );
        assert!(matches!(
            down.synthesize_stream("x", &prof(), &SynthOpts::default(), &cancel, &mut |_| {}),
            Err(VoiceError::NoEngine)
        ));
        let local = VoiceRouter::new(
            VoiceTier::LocalOnly,
            None,
            None,
            Some(Box::new(PcmOk)),
            Box::new(|| true),
        );
        assert!(matches!(
            local.synthesize_stream("x", &prof(), &SynthOpts::default(), &cancel, &mut |_| {}),
            Err(VoiceError::NoEngine)
        ));
    }

    #[test]
    fn allows_sidecar_matches_tier() {
        let auto = VoiceRouter::new(VoiceTier::Auto, None, None, None, Box::new(|| true));
        let local = VoiceRouter::new(VoiceTier::LocalOnly, None, None, None, Box::new(|| true));
        let side = VoiceRouter::new(VoiceTier::SidecarOnly, None, None, None, Box::new(|| true));
        assert!(auto.allows_sidecar());
        assert!(!local.allows_sidecar());
        assert!(side.allows_sidecar());
    }
}
