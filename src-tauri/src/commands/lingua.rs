//! LINGVÆTICA — диалоговый языковой тьютор (B-25, интейк #139).
//!
//! Методология Игоря Галкина, генерализованная с латино-профиля на сакральные
//! языки лаборатории: персона «не репетитор — поле языковой настройки»,
//! микро-циклы, речь прежде структуры, «ошибка = настройка», медиа-первичность
//! (для сакральных языков живой материал — мантры, шлоки, литургика, гурбани).
//!
//! LLM-шов тот же, что у course_feedback: активный post-process провайдер.
//! Диалог сворачивается в один user_content (API одноходовый) — для тьюторской
//! сессии этого достаточно, история короткая по построению (циклы ≤5 минут).

use tauri::AppHandle;

/// Языки тьютора (заказ владельца 2026-08-25). id — стабильный ключ стора,
/// label — как язык называется В ПРОМПТЕ (модель должна понять без словаря).
pub const LINGUA_LANGUAGES: &[(&str, &str)] = &[
    ("chu", "Church Slavonic (церковнославянский)"),
    ("san", "Sanskrit (санскрит, деванагари + IAST)"),
    ("arc", "Aramaic (арамейский, библейский/имперский)"),
    ("pli", "Pali (пали, тексты тхеравады)"),
    (
        "pan",
        "Gurmukhi Punjabi (панджаби письмом гурмукхи, язык гурбани)",
    ),
];

pub fn language_label(id: &str) -> Option<&'static str> {
    LINGUA_LANGUAGES
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
}

/// Персона тьютора. Русский — язык объяснений (язык владельца), материал —
/// на изучаемом языке с транслитерацией и переводом КАЖДОЙ формы: ученик не
/// обязан уже читать письменность, письменность — часть пути.
const LINGVAETICA_PERSONA: &str = "\
Ты — не репетитор. Ты — поле языковой настройки (метод LINGVÆTICA).
Твоя задача — не «преподавать язык», а вводить человека в живой ритм и звучание \
изучаемого языка и его традиции.

ОСНОВА: язык = состояние и движение; речь = резонанс; понимание = вхождение в поле. \
Не перегружать. Не дробить живое. Не объяснять ради объяснения.

ПРИНЦИПЫ КАЖДОГО ОТВЕТА:
- один цикл ≤ 5 минут чтения, ОДИН фокус за проход;
- сначала звучание и фраза — потом структура; грамматика только через живые паттерны;
- меньше терминов, больше узнавания; повтор через вариацию;
- ошибка ученика ≠ проблема: ошибка = настройка, отвечай мягкой поднастройкой;
- кратко, ритмично, без давления; пауза — часть смысла; переживание > заучивание.

АРХИТЕКТУРА ЦИКЛА (держи её, не называя вслух):
1 звук → 2 фраза → 3 ритм → 4 отклик ученика → 5 мини-диалог → 6 культурный нерв → 7 вариация.

МЕДИА-ПЕРВИЧНОСТЬ: живой материал вместо учебника. Для сакральных языков это \
мантры, шлоки, молитвы, литургические формулы, гурбани, строки писаний, пословицы — \
короткие подлинные фрагменты с дыханием традиции. Каждый фрагмент: оригинальное \
письмо + транслитерация + дословный и живой перевод.

ИЗБЕГАЙ: длинных правил, академического давления, таблиц-простыней, массивных \
списков слов, однообразия.

Объясняй по-русски. Материал давай на изучаемом языке (оригинальная письменность \
+ транслитерация + перевод). Заверши каждый цикл лёгким вопросом или micro-заданием, \
приглашающим ответить.";

#[tauri::command]
#[specta::specta]
pub async fn lingua_chat(
    app: AppHandle,
    language_id: String,
    profile: String,
    transcript: String,
    user_message: String,
) -> Result<String, String> {
    let lang = language_label(&language_id)
        .ok_or_else(|| format!("unknown lingua language: {language_id}"))?;

    let settings = crate::settings::get_settings(&app);
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

    let system = format!(
        "{LINGVAETICA_PERSONA}\n\nИЗУЧАЕМЫЙ ЯЗЫК: {lang}.\n\nЛИНГВОПРОФИЛЬ УЧЕНИКА \
         (анкета; настрой подачу под него, не пересказывай его):\n{profile}"
    );
    let user_content = if transcript.trim().is_empty() {
        format!("Ученик начинает сессию. Первое сообщение ученика: {user_message}")
    } else {
        format!(
            "Диалог сессии до этого момента:\n{transcript}\n\nНовое сообщение ученика: {user_message}"
        )
    };

    let reply = crate::llm_client::send_chat_completion_with_schema(
        &provider,
        api_key,
        &model,
        user_content,
        Some(system),
        None,
        None,
        None,
    )
    .await?;

    reply.ok_or_else(|| "empty reply from tutor".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_five_ordered_languages_are_registered() {
        for id in ["chu", "san", "arc", "pli", "pan"] {
            assert!(language_label(id).is_some(), "нет языка {id}");
        }
        assert_eq!(LINGUA_LANGUAGES.len(), 5);
        assert!(language_label("es").is_none());
    }

    #[test]
    fn persona_carries_the_method_pillars() {
        for pillar in [
            "поле языковой настройки",
            "ошибка = настройка",
            "гурбани",
            "транслитерация",
            "≤ 5 минут",
        ] {
            assert!(
                LINGVAETICA_PERSONA.contains(pillar),
                "персона потеряла опору: {pillar}"
            );
        }
    }
}
