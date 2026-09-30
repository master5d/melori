"""Prompting and tolerant parsing for client follow-up emails."""
from __future__ import annotations

from datetime import datetime

from melori_engine.council.language import with_language
from melori_engine.practice.models import NoteVersion
from melori_engine.practice.notes import ask_fields
from melori_engine.practice.templates import Section, Template


EMAIL_TEMPLATE = Template("email", "Email", True, (Section("subject", "Subject", ""),
                                                     Section("body", "Body", "")))


class EmailGenerationError(ValueError):
    pass


def note_text(note: NoteVersion) -> str:
    return "\n\n".join(f"{title}:\n{note.fields.get(key, '').strip()}"
                        for key, title in note.sections).strip()


def build_email_messages(note: str, language: str) -> list[dict]:
    messages = [
        {"role": "system", "content": (
            "You write a short follow-up email from the practitioner to their client after a session. "
            "Use only the agreements, next steps and homework present in the note. "
            "Do not include assessments, working formulations, diagnoses, analysis or any mention of a council or AI. "
            "Warm, businesslike tone, 80–200 words. Do not add facts that are not in the note."
        )},
        {"role": "user", "content": (
            f"Note:\n{note}\n\nAnswer with ONLY a JSON object, no markdown: "
            '{"subject": "...", "body": "..."}'
        )},
    ]
    return with_language(messages, language)


def _trim_body(value: str) -> str:
    if len(value) <= 4000:
        return value
    cut = value[:4000]
    boundary = max(cut.rfind(mark) for mark in (".", "!", "?"))
    return cut[:boundary + 1].rstrip() if boundary >= 0 else cut.rstrip()


def generate_email(llm, note: NoteVersion, language: str) -> tuple[dict[str, str], int]:
    try:
        fields, attempts = ask_fields(llm, build_email_messages(note_text(note), language), EMAIL_TEMPLATE)
    except Exception as exc:
        from melori_engine.practice.notes import NoteGenerationError
        if isinstance(exc, NoteGenerationError):
            raise EmailGenerationError(str(exc)) from exc
        raise
    return {"subject": fields["subject"][:120].rstrip(), "body": _trim_body(fields["body"])}, attempts
