//! Where a meeting analysis goes and whether the client's consent allows it.
//!
//! The analysis sends session content to a language model — for a client meeting that is
//! the `council` permission ("session analysis by a language model"), the same one the
//! council needs. The endpoint is melori's own LLM setting, not echo's post-processing
//! provider.

/// Endpoint and model for the analysis, from melori's LLM settings.
pub fn analysis_target(base_url: &str, model: &str) -> Result<(String, String), String> {
    let base = base_url.trim().trim_end_matches('/');
    let model = model.trim();
    if base.is_empty() || model.is_empty() {
        return Err(
            "language model is not configured: set the server address and model in Settings → Engine and language model"
                .into(),
        );
    }
    Ok((base.to_string(), model.to_string()))
}

/// `bound_permissions` is `None` outside a client meeting (no consent to check).
pub fn analysis_allowed(bound_permissions: Option<&[String]>) -> Result<(), String> {
    match bound_permissions {
        None => Ok(()),
        Some(perms) if perms.iter().any(|p| p == "council") => Ok(()),
        Some(_) => {
            Err("consent: this client has not allowed session analysis by a language model".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_comes_from_melori_settings() {
        assert_eq!(
            analysis_target("https://gw.example/v1/", "local-floor").unwrap(),
            (
                "https://gw.example/v1".to_string(),
                "local-floor".to_string()
            )
        );
    }

    #[test]
    fn unconfigured_llm_is_a_clear_error() {
        assert!(analysis_target("", "local-floor").is_err());
        assert!(analysis_target("http://127.0.0.1:11434/v1", " ").is_err());
    }

    #[test]
    fn client_meeting_needs_council_consent() {
        let with = vec!["transcript".to_string(), "council".to_string()];
        let without = vec!["transcript".to_string(), "retain".to_string()];
        assert!(analysis_allowed(Some(&with)).is_ok());
        let err = analysis_allowed(Some(&without)).unwrap_err();
        assert!(err.starts_with("consent:"));
    }

    #[test]
    fn a_meeting_without_a_client_has_no_consent_to_check() {
        assert!(analysis_allowed(None).is_ok());
    }
}
