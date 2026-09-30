"""Prompting and validation for structured practice notes."""
from __future__ import annotations

import inspect
import json
import re

from melori_engine.council.language import with_language
from melori_engine.practice.templates import Template


class NoteGenerationError(ValueError):
    pass


def build_messages(template: Template, transcript_text: str, language: str) -> list[dict]:
    sections = "\n".join(f"- {s.key}: {s.title} — {s.guidance}" for s in template.sections)
    shape = json.dumps({s.key: "..." for s in template.sections}, ensure_ascii=False)
    messages = [
        {"role": "system", "content": (
            "Document only what is explicitly said in the transcript. Do not invent findings, "
            "facts, diagnoses, or recommendations. Never name disorders, syndromes, diagnoses, "
            "or ICD codes; describe what the client said instead. Return the requested structured note."
        )},
        {"role": "user", "content": (
            f"Transcript:\n{transcript_text}\n\nSections to complete:\n{sections}\n"
            "Return one string value for every section key. Answer with ONLY a JSON object, "
            f"no markdown and no text around it, exactly in this shape: {shape}"
        )},
    ]
    return with_language(messages, language)


def schema_for(template: Template) -> dict:
    keys = [s.key for s in template.sections]
    return {"type": "object", "properties": {key: {"type": "string"} for key in keys},
            "required": keys, "additionalProperties": False}


def _complete(llm, messages, schema):
    response_format = {"type": "json_schema", "json_schema": {
        "name": "structured_note", "strict": True, "schema": schema,
    }}
    try:
        supports_format = "response_format" in inspect.signature(llm.complete).parameters
    except (TypeError, ValueError):
        supports_format = False
    return llm.complete(messages, response_format=response_format) if supports_format else llm.complete(messages)


def _json_object(raw: str):
    """A model without structured output may wrap the object in a ```json fence or a sentence."""
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        start, end = raw.find("{"), raw.rfind("}")
        if start < 0 or end <= start:
            raise
        return json.loads(raw[start:end + 1])


def _markdown_sections(raw: str, template: Template) -> dict | None:
    """local-floor ignores `response_format` and sometimes answers `**key**: text` or `## Title`:
    read those sections by key or by title; None unless at least one section is found."""
    names = {}
    for s in template.sections:
        names[s.key.lower()] = s.key
        names[s.title.lower()] = s.key
    heads = (re.compile(r"^\s*#{1,6}\s*(.+?)\s*:?\s*()$"),          # ## Title
             re.compile(r"^\s*\*\*(.+?):?\*\*\s*:?\s*(.*)$"),        # **key**: text
             re.compile(r"^\s*([^:*#]{1,60}):\s*(.*)$"))             # key: text
    out: dict[str, list[str]] = {}
    current = None
    for line in raw.splitlines():
        m = next((m for h in heads if (m := h.match(line)) and m.group(1).strip().lower() in names), None)
        if m:
            current = names[m.group(1).strip().lower()]
            out[current] = [m.group(2)] if m.group(2).strip() else []
        elif current is not None:
            out[current].append(line)
    return {k: "\n".join(v).strip() for k, v in out.items()} or None


def _as_text(value) -> str | None:
    if isinstance(value, str):
        return value
    if isinstance(value, list) and all(isinstance(x, (str, int, float)) for x in value):
        return "\n".join(f"- {x}" for x in value)
    return None


def _parse(raw, template: Template) -> dict:
    if isinstance(raw, dict):
        return raw
    text = str(raw)
    try:
        value = _json_object(text)
    except (TypeError, ValueError, json.JSONDecodeError):
        value = _markdown_sections(text, template)
    if not isinstance(value, dict):
        raise NoteGenerationError("invalid JSON")
    return value


def generate(llm, template: Template, transcript_text: str, language: str) -> tuple[dict[str, str], int]:
    messages = build_messages(template, transcript_text, language)
    return ask_fields(llm, messages, template)


def ask_fields(llm, messages: list[dict], template: Template) -> tuple[dict[str, str], int]:
    schema = schema_for(template)
    keys = [s.key for s in template.sections]
    last_error = NoteGenerationError("empty model response")
    for attempts in range(1, 4):
        raw = _complete(llm, messages, schema)
        if not raw or not str(raw).strip():
            continue
        try:
            value = _parse(raw, template)
        except NoteGenerationError as exc:
            last_error = exc  # unreadable reply: ask again, like an empty one
            continue
        missing = next((key for key in keys if key not in value), None)
        if missing:
            raise NoteGenerationError(f"missing section: {missing}")
        fields = {key: _as_text(value[key]) for key in keys}
        if any(v is None for v in fields.values()):
            raise NoteGenerationError("section values must be strings")
        return fields, attempts
    raise last_error
