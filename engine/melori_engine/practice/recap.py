"""The instant and model-generated client recap."""
from __future__ import annotations

import json
import re
from datetime import date, datetime
from pathlib import Path

from melori_engine.council.language import with_language
from melori_engine.practice import sessions, store


def _note_text(note) -> str:
    return "\n\n".join(f"{title}: {note.fields.get(key, '').strip()}" for key, title in note.sections).strip()


def quick_recap(root, cid: str, today: date | None = None) -> dict | None:
    today = today or date.today()
    closed = sorted((s for s in store.list_sessions(root, cid) if s.status == "closed"), key=lambda s: (s.date, s.id))
    if not closed:
        return None
    session = closed[-1]
    note = session.notes[-1] if session.notes else None
    if note:
        preferred = next((key for key in ("plan", "will") if any(k == key for k, _ in note.sections)), None)
        if preferred is None and note.sections:
            preferred = note.sections[-1][0]
        section = next(((title, note.fields.get(key, "")) for key, title in note.sections if key == preferred), None)
        plan = {"title": section[0], "text": section[1]} if section else None
    else:
        plan = None
    email = sessions.read_email(sessions.session_dir(root, cid, session.id))
    return {"session_id": session.id, "date": session.date,
            "days_ago": max(0, (today - date.fromisoformat(session.date)).days),
            "plan": plan, "email_subject": email.get("subject") if email else None}


def _points(raw) -> list[str]:
    text = str(raw or "").strip()
    try:
        value = json.loads(text)
    except (TypeError, ValueError, json.JSONDecodeError):
        start, end = text.find("{"), text.rfind("}")
        if start >= 0 and end > start:
            try:
                value = json.loads(text[start:end + 1])
            except json.JSONDecodeError:
                value = None
        else:
            value = None
    if isinstance(value, dict):
        # "points" as asked; local-floor has answered {"summary": [...]} — take the one list it gave
        lists = [v for v in value.values() if isinstance(v, list)]
        items = value["points"] if isinstance(value.get("points"), list) else (lists[0] if len(lists) == 1 else None)
        if items is not None:
            return [str(x).strip() for x in items if str(x).strip()]
    return [m.group(1).strip() for m in re.finditer(r"^\s*(?:[-*•]|\d+[.)])\s+(.+?)\s*$", text, re.M)]


class RecapError(ValueError):
    pass


def model_recap(llm, notes: list[tuple[str, str]], language: str) -> tuple[list[str], int]:
    messages = with_language([
        {"role": "system", "content": (
            "You prepare a practitioner for the next session with their client. From the supplied session notes only, "
            "write 3 to 5 short points: agreements and homework, open themes, what to check at the next meeting. "
            "No diagnoses, no speculation, nothing that is not in the notes. "
            'Answer with ONLY a JSON object, no markdown: {"points": ["...", "..."]}')},
        {"role": "user", "content": "\n\n".join(f"[{d}]\n{text}" for d, text in notes)},
    ], language)
    for attempts in range(1, 4):
        points = _points(llm.complete(messages) or "")
        if points:
            return points[:5], attempts
    raise RecapError("the model returned no points")


def _path(root, cid: str) -> Path:
    return Path(root) / cid / "recap.json"


def read_recap(root, cid: str) -> dict | None:
    path = _path(root, cid)
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else None


def write_recap(root, cid: str, value: dict) -> None:
    path = _path(root, cid)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def recap_stale(recap: dict | None, sessions) -> bool:
    if not recap:
        return False
    known = set(recap.get("sessions", []))
    return any(s.status == "closed" and s.id not in known for s in sessions)
