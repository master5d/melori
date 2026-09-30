"""One-way migration of v1 Markdown sessions to v2 directories."""
from __future__ import annotations

import json
import shutil
from pathlib import Path

import yaml

from melori_engine.practice import sessions


def _frontmatter(raw: str) -> tuple[dict, str]:
    raw = raw.replace("\r\n", "\n")
    if not raw.startswith("---"):
        raise ValueError("missing frontmatter")
    parts = raw.split("\n---", 1)
    if len(parts) != 2:
        raise ValueError("malformed frontmatter")
    meta = yaml.safe_load(parts[0][3:].lstrip("\n")) or {}
    if not isinstance(meta, dict) or "id" not in meta or "client" not in meta or "date" not in meta:
        raise ValueError("invalid session metadata")
    return meta, parts[1].lstrip("\n")


def _transcript(body: str):
    marker = body.find("# Transcript")
    if marker < 0:
        return []
    tail = body[marker + len("# Transcript"):]
    end = tail.find("\n# ")
    text = tail if end < 0 else tail[:end]
    out = []
    for i, line in enumerate(text.splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        if ":" in line and line.split(":", 1)[0].strip() in {"me", "others"}:
            source, value = line.split(":", 1)
            out.append({"i": i, "source": source.strip(), "start_ms": 0, "end_ms": 0, "text": value.strip()})
        else:
            out.append({"i": i, "source": "others", "start_ms": 0, "end_ms": 0, "text": line})
    return out


def _note(body: str) -> tuple[str, dict[str, str], tuple[tuple[str, str], ...]]:
    soap = body.find("# SOAP")
    if soap < 0:
        return "free", {"text": body}, (("text", "Заметка"),)
    tail = body[soap + len("# SOAP"):]
    end = tail.find("\n# ")
    tail = tail if end < 0 else tail[:end]
    fields = {}
    sections = []
    current = None
    for line in tail.splitlines():
        if line.startswith("## "):
            current = line[3:].strip()
            continue
        if current is not None:
            fields.setdefault(current.lower(), "")
            fields[current.lower()] += ("\n" if fields[current.lower()] else "") + line
    names = [("subjective", "Subjective"), ("objective", "Objective"),
             ("assessment", "Assessment"), ("plan", "Plan")]
    if not any(k in fields for k, _ in names):
        return "free", {"text": body}, (("text", "Заметка"),)
    normalized = {k: fields.get(k, "").strip() for k, _ in names}
    return "soap", normalized, tuple(names)


def migrate_session_file(path: Path, dir: Path, alias: str | None = None) -> None:
    raw_bytes = path.read_bytes()
    meta, body = _frontmatter(raw_bytes.decode("utf-8"))
    d = Path(dir)
    d.mkdir(parents=True, exist_ok=False)
    try:
        migrated = dict(meta)
        migrated.update({"schema": 2, "migrated_from": "v1", "client_alias": alias or meta.get("client", "")})
        migrated.pop("runs", None)
        sessions.write_meta(d, migrated)
        segments = _transcript(body)
        if segments:
            sessions._atomic_write(d / "transcript.jsonl", "".join(json.dumps(x, ensure_ascii=False) + "\n" for x in segments))
        template_id, fields, sections = _note(body)
        sessions.add_note(d, author="import", template_id=template_id, fields=fields,
                          sections=sections, parent=None)
        for run in meta.get("runs") or []:
            sessions.add_run(d, dict(run))
        (d / "legacy.md").write_bytes(raw_bytes)
        sessions.rebuild_session_md(d, migrated["client_alias"])
    except BaseException:
        shutil.rmtree(d, ignore_errors=True)
        raise
    path.unlink()  # only once the v2 directory is complete: a failed step leaves the source untouched


def migrate_client(root, client_id: str) -> dict:
    base = Path(root) / client_id / "sessions"
    if not base.is_dir():
        return {"migrated": 0, "failed": []}
    result = {"migrated": 0, "failed": []}
    from melori_engine.practice.store import get_client
    client = get_client(root, client_id)
    alias = client.alias if client else None
    for path in sorted(base.glob("*.md")):
        if path.name == "session.md":
            continue
        sid = path.stem
        target = base / sid
        if target.is_dir() and (target / "meta.json").exists():
            continue
        try:
            migrate_session_file(path, target, alias)
            result["migrated"] += 1
        except Exception:
            result["failed"].append(sid)
    return result
