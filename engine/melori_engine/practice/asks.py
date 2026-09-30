"""Questions answered from the transcript of the current meeting."""
from __future__ import annotations

from melori_engine.council.language import with_language

QUICK = {
    "recap5": {"ru": "Кратко: о чём говорили последние 5 минут?", "en": "Briefly: what was discussed in the last 5 minutes?"},
    "missed": {"ru": "Что важное из сказанного клиентом я мог упустить?", "en": "What important thing the client said might I have missed?"},
    "agreed": {"ru": "К чему мы договорились на этой встрече?", "en": "What did we agree on in this meeting?"},
}
NOTHING_SAID = {"ru": "пока ничего не сказано", "en": "nothing has been said yet"}

class AskError(ValueError):
    pass

def transcript_for_ask(segments: list[dict], kind: str, elapsed_ms: int, limit: int = 12000) -> tuple[str, bool]:
    cutoff = elapsed_ms - 300_000
    rows = [s for s in segments if s.get("source") != "mark" and (kind != "recap5" or int(s.get("end_ms", 0)) >= cutoff) and str(s.get("text", "")).strip()]
    lines = [("Я" if s.get("source") == "me" else "Клиент") + ": " + str(s["text"]).strip() for s in rows]
    text = "\n".join(lines)
    return (text, False) if len(text) <= limit else (text[-limit:], True)

def build_ask_messages(question, text, truncated, language) -> list[dict]:
    messages = [
        {"role": "system", "content": "Answer only from the meeting transcript. Do not diagnose or name disorders, syndromes, diagnoses, or ICD codes. Do not provide what to say, scripts, or advice to the practitioner about what to say to the client. If the transcript does not answer the question, say so. Answer in 3–6 short lines."},
        {"role": "user", "content": f"Transcript:\n{text}\n\nQuestion: {question}"},
    ]
    if truncated:
        messages[-1]["content"] += "\n\nThe beginning of the meeting was omitted."
    return with_language(messages, language)

def ask(llm, question, kind, segments, elapsed_ms, language) -> tuple[str, dict]:
    text, truncated = transcript_for_ask(segments, kind, elapsed_ms)
    if not text:
        return NOTHING_SAID.get(language, NOTHING_SAID["en"]), {"input": "transcript", "chars_sent": 0, "until_ms": elapsed_ms, "truncated": False, "language": language, "attempts": 0}
    messages = build_ask_messages(question, text, truncated, language)
    for attempts in range(1, 4):
        answer = llm.complete(messages)
        if answer and str(answer).strip():
            return str(answer).strip(), {"input": "transcript", "chars_sent": len(text), "until_ms": elapsed_ms, "truncated": truncated, "language": language, "attempts": attempts}
    raise AskError("empty model response")
