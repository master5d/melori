use tauri::AppHandle;

pub const DEFAULT_ASSISTANT_SYSTEM_PROMPT: &str = "You are Echo, a concise, friendly voice assistant. Answer in the same language the user used (Russian or English). Keep replies short and speakable — no markdown, no code fences, no bullet lists unless asked.";

pub async fn ask_assistant(app: &AppHandle, user_text: String) -> Result<String, String> {
    let settings = crate::settings::get_settings(app);

    let provider = settings
        .active_post_process_provider()
        .ok_or_else(|| "no LLM provider configured".to_string())?
        .clone();

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.is_empty() {
        return Err("no LLM model configured for the active provider".to_string());
    }

    let system = if settings.assistant_system_prompt.is_empty() {
        DEFAULT_ASSISTANT_SYSTEM_PROMPT.to_string()
    } else {
        settings.assistant_system_prompt.clone()
    };

    let reply = crate::llm_client::send_chat_completion_with_schema(
        &provider,
        api_key,
        &model,
        user_text,
        Some(system),
        None,
        None,
        None,
    )
    .await?;

    reply.ok_or_else(|| "empty reply from assistant".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_prompt() {
        assert!(!DEFAULT_ASSISTANT_SYSTEM_PROMPT.is_empty());
        assert!(DEFAULT_ASSISTANT_SYSTEM_PROMPT.contains("Echo"));
    }
}
