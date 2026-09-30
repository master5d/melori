"""Turn one v2 session into searchable chunks."""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from melori_engine.practice import sessions


@dataclass(frozen=True)
class Chunk:
    kind: str
    label: str
    start_ms: int | None
    text: str


def _transcript_chunks(directory: Path) -> list[Chunk]:
    rows = sessions.read_transcript(directory)
    out: list[Chunk] = []
    current: list = []
    size = 0
    for row in rows:
        if row.source == "mark":
            continue
        speaker = "Я" if row.source == "me" else "Клиент"
        line = f"{speaker}: {row.text}"
        proposed = size + (1 if current else 0) + len(line)
        if current and proposed > 600:
            out.append(Chunk("transcript", "", current[0].start_ms, "\n".join(current_lines)))
            current = current[-1:]
            current_lines = [current_lines[-1]]
            size = len(current_lines[0])
        elif not current:
            current_lines = []
        current.append(row)
        current_lines.append(line)
        size += (1 if len(current_lines) > 1 else 0) + len(line)
    if current:
        out.append(Chunk("transcript", "", current[0].start_ms, "\n".join(current_lines)))
    return out


def chunks_for_session(directory: Path) -> list[Chunk]:
    directory = Path(directory)
    out = _transcript_chunks(directory)
    notes = sessions.list_notes(directory)
    if notes:
        current = notes[-1]
        out.extend(Chunk("note", title, None, current.fields.get(key, ""))
                   for key, title in current.sections
                   if current.fields.get(key, "").strip())
    email = sessions.read_email(directory)
    if email:
        subject = str(email.get("subject", ""))
        body = str(email.get("body", ""))
        out.append(Chunk("email", subject, None, f"{subject}\n{body}".strip()))
    return out
