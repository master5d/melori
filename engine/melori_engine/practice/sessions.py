"""Part-based v2 session storage."""
from __future__ import annotations

import json
import os
import time
from datetime import datetime
from pathlib import Path

from melori_engine.practice.models import CouncilRun, NoteVersion, Segment


def _atomic_write(path: Path, text: str) -> None:
    path = Path(path)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        tmp.write_text(text, encoding="utf-8")
        os.replace(tmp, path)
    finally:
        tmp.unlink(missing_ok=True)


def session_dir(root, cid: str, sid: str) -> Path:
    return Path(root) / cid / "sessions" / sid


def _json_write(path: Path, value) -> None:
    _atomic_write(path, json.dumps(value, ensure_ascii=False, indent=2, default=str) + "\n")


def write_meta(dir, meta: dict) -> None:
    Path(dir).mkdir(parents=True, exist_ok=True)
    _json_write(Path(dir) / "meta.json", meta)


def read_meta(dir) -> dict:
    return json.loads((Path(dir) / "meta.json").read_text(encoding="utf-8"))


def write_transcript(dir, segments: list[Segment]) -> None:
    text = "".join(json.dumps({"i": s.i, "source": s.source, "start_ms": s.start_ms,
                                "end_ms": s.end_ms, "text": s.text}, ensure_ascii=False) + "\n"
                   for s in segments)
    _atomic_write(Path(dir) / "transcript.jsonl", text)


def read_transcript(dir) -> list[Segment]:
    path = Path(dir) / "transcript.jsonl"
    if not path.exists():
        return []
    return [Segment(int(d["i"]), str(d["source"]), int(d["start_ms"]), int(d["end_ms"]), str(d["text"]))
            for line in path.read_text(encoding="utf-8").splitlines() if line.strip()
            for d in [json.loads(line)]]


def read_email(dir) -> dict | None:
    path = Path(dir) / "email.json"
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else None


def write_email(dir, email: dict) -> None:
    Path(dir).mkdir(parents=True, exist_ok=True)
    _json_write(Path(dir) / "email.json", email)


def _next_exclusive(directory: Path, suffix: str) -> tuple[int, Path]:
    directory.mkdir(parents=True, exist_ok=True)
    n = 1
    while True:
        path = directory / f"{n:03d}{suffix}"
        try:
            fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.close(fd)
            return n, path
        except FileExistsError:
            n += 1


def add_note(dir, *, author, template_id, fields, sections, parent, provenance=None, warnings=()) -> NoteVersion:
    d = Path(dir)
    n, path = _next_exclusive(d / "notes", ".json")
    value = {"n": n, "created": datetime.now().isoformat(timespec="seconds"), "author": author,
             "template_id": template_id, "fields": dict(fields), "parent": parent,
             "provenance": provenance, "sections": [list(s) for s in sections], "warnings": list(warnings)}
    try:
        _atomic_write(path, json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    except BaseException:
        path.unlink(missing_ok=True)
        raise
    return _note(value)


def _note(value: dict) -> NoteVersion:
    return NoteVersion(n=int(value["n"]), created=str(value["created"]), author=str(value["author"]),
                       template_id=str(value["template_id"]), fields=dict(value.get("fields") or {}),
                       parent=value.get("parent"), provenance=value.get("provenance"),
                       sections=tuple((str(k), str(v)) for k, v in value.get("sections") or []),
                       warnings=tuple(value.get("warnings") or []))


def read_note(dir, n: int) -> NoteVersion:
    return _note(json.loads((Path(dir) / "notes" / f"{n:03d}.json").read_text(encoding="utf-8")))


def list_notes(dir) -> list[NoteVersion]:
    notes = Path(dir) / "notes"
    return [_note(json.loads(p.read_text(encoding="utf-8"))) for p in sorted(notes.glob("*.json"))]


def add_run(dir, run: dict) -> int:
    n, path = _next_exclusive(Path(dir) / "runs", ".json")
    value = dict(run)
    value.setdefault("index", n)
    try:
        _atomic_write(path, json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    except BaseException:
        path.unlink(missing_ok=True)
        raise
    return n


def read_runs(dir) -> list[CouncilRun]:
    runs = Path(dir) / "runs"
    return [CouncilRun(index=int(d["index"]), timestamp=str(d["timestamp"]), mode=str(d["mode"]),
                       specialist_ids=list(d.get("specialist_ids") or []), result=dict(d.get("result") or {}))
            for p in sorted(runs.glob("*.json"))
            for d in [json.loads(p.read_text(encoding="utf-8"))]]


def read_asks(dir) -> list[dict]:
    path = Path(dir) / "asks.jsonl"
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()] if path.exists() else []


def append_ask(dir, ask: dict) -> None:
    d = Path(dir)
    d.mkdir(parents=True, exist_ok=True)
    lock = d / ".lock"
    deadline = time.monotonic() + 5
    while True:
        try:
            fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.close(fd)
            break
        except FileExistsError:
            if time.monotonic() >= deadline:
                raise TimeoutError(f"session lock timeout: {d}")
            time.sleep(0.02)
    try:
        with (d / "asks.jsonl").open("a", encoding="utf-8", newline="\n") as fh:
            fh.write(json.dumps(ask, ensure_ascii=False) + "\n")
    finally:
        lock.unlink(missing_ok=True)


def note_markdown(note: NoteVersion | None) -> str:
    """The current note alone, as Markdown: what export and the council read as `Session.note`."""
    if note is None:
        return ""
    if [k for k, _ in note.sections] == ["text"]:
        return note.fields.get("text", "").strip()
    return "\n\n".join(f"## {title}\n\n{note.fields.get(key, '').strip()}" for key, title in note.sections).strip()


def render_markdown(meta, transcript, current_note, runs, alias, email=None) -> str:
    lines = [f"# {alias} — {meta.get('date', '')}", "", f"Статус: {meta.get('status', '')}",
             f"Шаблон: {meta.get('template_id') or 'soap'}", ""]
    if current_note:
        lines += ["## Заметка", ""]
        for key, title in current_note.sections:
            lines += [f"### {title}", "", current_note.fields.get(key, ""), ""]
    if transcript:
        lines += ["## Transcript", ""]
        lines += [f"{s.source}: {s.text}" for s in transcript] + [""]
    if runs:
        lines += ["## Разбор", "", str(runs[-1].result.get("synthesis", {}).get("text", "")), ""]
    if email:
        lines += ["## Письмо клиенту", "", f"**{email.get('subject', '')}**", "", email.get("body", ""), ""]
    return "\n".join(lines).rstrip() + "\n"


def rebuild_session_md(dir, alias) -> None:
    d = Path(dir)
    lock = d / ".lock"
    deadline = time.monotonic() + 5
    while True:
        try:
            fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.close(fd)
            break
        except FileExistsError:
            if time.monotonic() >= deadline:
                raise TimeoutError(f"session lock timeout: {d}")
            time.sleep(0.02)
    try:
        meta = read_meta(d)
        notes = list_notes(d)
        _atomic_write(d / "session.md", render_markdown(meta, read_transcript(d), notes[-1] if notes else None,
                                                          read_runs(d), alias, read_email(d)))
    finally:
        lock.unlink(missing_ok=True)
