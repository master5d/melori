# Vendored from wellbeing practice/ @ bd50668 (2026-09-27); diverges: scoped consent, delete, retain.
"""Markdown file I/O for the gitignored client/sessions PHI domain."""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import tempfile
from datetime import date as _date, datetime, timedelta
from pathlib import Path

import yaml

from melori_engine.practice.models import PERMISSIONS, Client, Consent, CouncilRun, Session
from melori_engine.practice import sessions as session_store

_CLIENT_ID = re.compile(r"^[a-z0-9][a-z0-9-]*$")
_SESSION_ID = re.compile(r"^\d{4}-\d{2}-\d{2}-\d{2}$")

_RU_TRANSLIT = {
    "а": "a", "б": "b", "в": "v", "г": "g", "д": "d", "е": "e", "ё": "e",
    "ж": "zh", "з": "z", "и": "i", "й": "i", "к": "k", "л": "l", "м": "m",
    "н": "n", "о": "o", "п": "p", "р": "r", "с": "s", "т": "t", "у": "u",
    "ф": "f", "х": "kh", "ц": "ts", "ч": "ch", "ш": "sh", "щ": "shch",
    "ъ": "", "ы": "y", "ь": "", "э": "e", "ю": "iu", "я": "ia",
}


class StoreError(ValueError):
    """Bad id / not found / unreadable store entry."""


class ConsentError(ValueError):
    """Action blocked because the client has not given consent."""


def _transliterate(s: str) -> str:
    return "".join(_RU_TRANSLIT.get(ch, ch) for ch in s.lower())


def slugify_alias(alias: str) -> str:
    if not alias.strip():
        raise StoreError("alias does not yield a usable slug")
    slug = re.sub(r"[^a-z0-9]+", "-", _transliterate(alias.strip())).strip("-")
    if not slug:                       # non-empty alias with no latinizable chars (e.g. CJK/emoji)
        # non-cryptographic fingerprint only (sha256 to satisfy the security linter)
        slug = "client-" + hashlib.sha256(alias.encode("utf-8")).hexdigest()[:8]
    return slug


def _atomic_write(path: Path, text: str) -> None:
    """Write ``text`` to ``path`` atomically: temp file in the same dir + os.replace.
    On any failure the target is left untouched and the temp file is removed."""
    path = Path(path)
    fd, tmp_name = tempfile.mkstemp(dir=str(path.parent), prefix=path.name + ".", suffix=".tmp")
    os.close(fd)
    tmp = Path(tmp_name)
    try:
        tmp.write_text(text, encoding="utf-8")
        os.replace(tmp, path)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise


def _check_client_id(client_id: str) -> None:
    if not _CLIENT_ID.match(client_id):
        raise StoreError(f"unsafe client id: {client_id!r}")


def _check_session_id(session_id: str) -> None:
    if not _SESSION_ID.match(session_id):
        raise StoreError(f"unsafe session id: {session_id!r}")


def _split_frontmatter(text: str) -> tuple[dict, str]:
    if not text.startswith("---"):
        raise StoreError("missing frontmatter")
    m = re.match(r"^---\n(.*?)\n---\n?(.*)\Z", text, re.DOTALL)
    if not m:
        raise StoreError("malformed frontmatter")
    meta = yaml.safe_load(m.group(1)) or {}
    if not isinstance(meta, dict):
        raise StoreError("frontmatter is not a mapping")
    return meta, m.group(2).lstrip("\n")


def _client_dir(root, client_id: str) -> Path:
    _check_client_id(client_id)
    return Path(root) / client_id


def _consent_to_meta(c: Consent) -> dict:
    return {"permissions": sorted(c.permissions), "date": c.date, "retain_days": c.retain_days,
            "template_version": c.template_version, "signed_sha256": c.signed_sha256,
            "revoked": c.revoked}


def _consent_from_meta(m: dict) -> Consent:
    return Consent(permissions=frozenset(m.get("permissions") or []), date=m.get("date"),
                   retain_days=m.get("retain_days"), template_version=m.get("template_version"),
                   signed_sha256=m.get("signed_sha256"), revoked=m.get("revoked"))


def _render_client(c: Client) -> str:
    meta = {"id": c.id, "alias": c.alias, "tags": list(c.tags), "created": c.created,
            "consent": _consent_to_meta(c.consent), "template_id": c.template_id}
    return "---\n" + yaml.safe_dump(meta, allow_unicode=True, sort_keys=False) + "---\n" + (c.intake or "")


def _parse_client(text: str) -> Client:
    meta, body = _split_frontmatter(text)
    return Client(id=str(meta["id"]), alias=str(meta["alias"]), tags=list(meta.get("tags") or []),
                  consent=_consent_from_meta(meta.get("consent") or {}),
                  created=str(meta.get("created", "")), intake=body, template_id=meta.get("template_id"))


def _write_client(root, c: Client) -> None:
    d = _client_dir(root, c.id)
    d.mkdir(parents=True, exist_ok=True)
    _atomic_write(d / "client.md", _render_client(c))


def _validate_permissions(perms) -> frozenset[str]:
    unknown = set(perms) - set(PERMISSIONS)
    if unknown:
        raise StoreError(f"unknown permissions: {sorted(unknown)}")
    return frozenset(perms)


def create_client(root, alias: str, *, tags: list[str], consent: Consent, intake: str = "") -> Client:
    cid = slugify_alias(alias)
    if (_client_dir(root, cid) / "client.md").exists():
        raise StoreError(f"client already exists: {cid}")
    consent = Consent(permissions=_validate_permissions(consent.permissions), date=consent.date,
                      retain_days=consent.retain_days, template_version=consent.template_version)
    c = Client(id=cid, alias=alias, tags=list(tags), consent=consent,
               created=datetime.now().isoformat(timespec="seconds"), intake=intake, template_id=None)
    _write_client(root, c)
    return c


def get_client(root, client_id: str) -> Client | None:
    p = _client_dir(root, client_id) / "client.md"
    if not p.exists():
        return None
    return _parse_client(p.read_text(encoding="utf-8"))


def list_clients(root) -> list[Client]:
    root = Path(root)
    if not root.is_dir():
        return []
    out = []
    for d in sorted(root.iterdir()):
        if d.is_dir() and _CLIENT_ID.match(d.name) and (d / "client.md").exists():
            out.append(_parse_client((d / "client.md").read_text(encoding="utf-8")))
    return out


def _must_client(root, client_id: str) -> Client:
    c = get_client(root, client_id)
    if c is None:
        raise StoreError(f"no such client: {client_id}")
    return c


def set_client_template(root, client_id: str, template_id: str | None) -> Client:
    c = _must_client(root, client_id)
    updated = Client(id=c.id, alias=c.alias, tags=c.tags, consent=c.consent, created=c.created,
                     intake=c.intake, template_id=template_id)
    _write_client(root, updated)
    return updated


def require_permission(root, client_id: str, perm: str) -> Client:
    if perm not in PERMISSIONS:
        raise StoreError(f"unknown permission: {perm}")
    c = _must_client(root, client_id)
    if c.consent.revoked is not None:
        raise ConsentError(f"consent revoked for client {client_id}")
    if perm not in c.consent.permissions:
        raise ConsentError(f"permission {perm!r} not granted for client {client_id}")
    return c


def revoke(root, client_id: str, *, today: str) -> Client:
    c = _must_client(root, client_id)
    co = c.consent
    new = Client(id=c.id, alias=c.alias, tags=c.tags, created=c.created, intake=c.intake,
                 consent=Consent(permissions=co.permissions, date=co.date, retain_days=co.retain_days,
                                 template_version=co.template_version,
                                 signed_sha256=co.signed_sha256, revoked=today), template_id=c.template_id)
    _write_client(root, new)
    return new


def delete_client(root, client_id: str) -> None:
    d = _client_dir(root, client_id)
    if not d.exists():
        raise StoreError(f"no such client: {client_id}")
    shutil.rmtree(d)


def attach_signed_consent(root, client_id: str, *, data: bytes, template_version: str) -> Client:
    c = _must_client(root, client_id)
    digest = hashlib.sha256(data).hexdigest()
    d = _client_dir(root, client_id)
    (d / "consent").mkdir(exist_ok=True)
    (d / "consent" / f"signed-{template_version}-{digest[:12]}.bin").write_bytes(data)
    co = c.consent
    new = Client(id=c.id, alias=c.alias, tags=c.tags, created=c.created, intake=c.intake,
                 consent=Consent(permissions=co.permissions, date=co.date, retain_days=co.retain_days,
                                 template_version=template_version, signed_sha256=digest,
                                 revoked=co.revoked), template_id=c.template_id)
    _write_client(root, new)
    return new


def _sessions_dir(root, client_id: str) -> Path:
    return _client_dir(root, client_id) / "sessions"


def _session_file(root, client_id: str, session_id: str) -> Path:
    _check_session_id(session_id)
    return _sessions_dir(root, client_id) / f"{session_id}.md"


def _render_session(s: Session) -> str:
    meta = {"id": s.id, "client": s.client, "date": s.date, "created": s.created,
            "status": s.status, "meeting_type": s.meeting_type,
            "runs": [{"index": r.index, "timestamp": r.timestamp, "mode": r.mode,
                      "specialist_ids": list(r.specialist_ids), "result": r.result} for r in s.runs]}
    return "---\n" + yaml.safe_dump(meta, allow_unicode=True, sort_keys=False) + "---\n" + (s.note or "")


def _parse_session(text: str) -> Session:
    meta, body = _split_frontmatter(text)
    runs = [CouncilRun(index=int(r["index"]), timestamp=str(r["timestamp"]), mode=str(r["mode"]),
                       specialist_ids=list(r.get("specialist_ids") or []), result=dict(r.get("result") or {}))
            for r in (meta.get("runs") or [])]
    return Session(id=str(meta["id"]), client=str(meta["client"]), date=str(meta["date"]),
                   created=str(meta.get("created", "")), status=meta.get("status", "closed"),
                   meeting_type=str(meta.get("meeting_type", "session")), note=body, runs=runs)


def _session_from_dir(d: Path) -> Session:
    meta = session_store.read_meta(d)
    transcript = session_store.read_transcript(d)
    notes = session_store.list_notes(d)
    runs = session_store.read_runs(d)
    md = d / "session.md"
    newest = max((p.stat().st_mtime for p in d.rglob("*") if p.is_file() and p.name != ".lock"), default=0)
    if not md.exists() or md.stat().st_mtime < newest:
        session_store.rebuild_session_md(d, str(meta.get("client_alias", meta.get("client", ""))))
    note = session_store.note_markdown(notes[-1] if notes else None)
    return Session(id=str(meta["id"]), client=str(meta["client"]), date=str(meta["date"]),
                   created=str(meta.get("created", "")), status=meta.get("status", "closed"),
                   meeting_type=str(meta.get("meeting_type", "session")), note=note, runs=runs,
                   template_id=meta.get("template_id"), duration_ms=int(meta.get("duration_ms", 0)),
                   language=meta.get("language"), transcript=transcript, notes=notes,
                   asks=session_store.read_asks(d), migrated=bool(meta.get("migrated_from")))


def _legacy_or_dir(root, client_id: str, session_id: str) -> Path:
    return session_store.session_dir(root, client_id, session_id)


def _unmigrated(p: Path, client_id: str) -> Session:
    """A v1 file the migration could not convert: shown as-is, never rewritten."""
    text = p.read_text(encoding="utf-8")
    try:
        meta, body = _split_frontmatter(text)
        return Session(id=str(meta["id"]), client=str(meta["client"]), date=str(meta["date"]),
                       created=str(meta.get("created", "")), status="unmigrated",
                       meeting_type=str(meta.get("meeting_type", "session")), note=body, runs=[])
    except Exception:
        return Session(id=p.stem, client=client_id, date=p.stem[:10], created="",
                       status="unmigrated", meeting_type="session", note=text)


def get_session(root, client_id: str, session_id: str) -> Session | None:
    from melori_engine.practice.migrate import migrate_client
    migrate_client(root, client_id)
    d = _legacy_or_dir(root, client_id, session_id)
    if d.is_dir() and (d / "meta.json").exists():
        return _session_from_dir(d)
    p = _session_file(root, client_id, session_id)
    return _unmigrated(p, client_id) if p.exists() else None


def list_sessions(root, client_id: str) -> list[Session]:
    from melori_engine.practice.migrate import migrate_client
    migrate_client(root, client_id)
    d = _sessions_dir(root, client_id)
    if not d.is_dir():
        return []
    out = []
    for p in sorted(d.iterdir()):
        if p.is_dir() and _SESSION_ID.match(p.name) and (p / "meta.json").exists():
            out.append(_session_from_dir(p))
        elif p.is_file() and p.suffix == ".md" and _SESSION_ID.match(p.stem):
            out.append(_unmigrated(p, client_id))
    return out


def delete_session(root, client_id: str, session_id: str) -> None:
    """Remove one session file. No consent check: deleting data is always allowed,
    including after revocation."""
    path = _legacy_or_dir(root, client_id, session_id)
    legacy = _session_file(root, client_id, session_id)
    if not path.exists() and not legacy.exists():
        raise StoreError(f"no such session: {client_id}/{session_id}")
    index_path = _client_dir(root, client_id) / "index.sqlite"
    if index_path.exists():
        from melori_engine.practice.index import ClientIndex
        ix = ClientIndex(root, client_id)
        try:
            ix.drop_session(session_id, keep_record=True)
        finally:
            ix.close()
    if path.is_dir():
        shutil.rmtree(path)
    else:
        legacy.unlink()


def open_session(root, client_id: str, *, date: str, meeting_type: str) -> Session:
    require_permission(root, client_id, "transcript")
    _date.fromisoformat(date)
    d = _sessions_dir(root, client_id)
    d.mkdir(parents=True, exist_ok=True)
    n = 1
    while (d / f"{date}-{n:02d}").exists() or (d / f"{date}-{n:02d}.md").exists():
        n += 1
    if n > 99:
        raise StoreError("too many sessions on one day")
    s = Session(id=f"{date}-{n:02d}", client=client_id, date=date,
                created=datetime.now().isoformat(timespec="seconds"), status="open",
                meeting_type=meeting_type, note="", runs=[], template_id=get_client(root, client_id).template_id)
    sd = session_store.session_dir(root, client_id, s.id)
    sd.mkdir(parents=True)
    session_store.write_meta(sd, {"schema": 2, "id": s.id, "client": client_id, "client_alias": get_client(root, client_id).alias,
                                  "date": date, "created": s.created, "status": "open", "meeting_type": meeting_type,
                                  "template_id": s.template_id, "duration_ms": 0, "language": None})
    session_store.rebuild_session_md(sd, get_client(root, client_id).alias)
    return s


def close_session(root, client_id: str, session_id: str, *, note: str) -> tuple[Session | None, bool]:
    c = require_permission(root, client_id, "transcript")
    s = get_session(root, client_id, session_id)
    if s is None:
        raise StoreError(f"no such session: {client_id}/{session_id}")
    p = _legacy_or_dir(root, client_id, session_id)
    if "retain" not in c.consent.permissions:
        if p.is_dir():
            shutil.rmtree(p)
        _session_file(root, client_id, session_id).unlink(missing_ok=True)
        return None, False
    if s.status == "unmigrated":
        raise StoreError(f"session is not migrated: {client_id}/{session_id}")
    d = p
    meta = session_store.read_meta(d)
    meta["status"] = "closed"
    session_store.write_meta(d, meta)
    if note:
        session_store.add_note(d, author="practitioner", template_id="free", fields={"text": note},
                               sections=(("text", "Заметка"),), parent=None)
    session_store.rebuild_session_md(d, c.alias)
    stored = _session_from_dir(d)
    return Session(id=stored.id, client=stored.client, date=stored.date, created=stored.created,
                   status=stored.status, meeting_type=stored.meeting_type, note=note, runs=stored.runs,
                   template_id=stored.template_id, duration_ms=stored.duration_ms, language=stored.language,
                   transcript=stored.transcript, notes=stored.notes, asks=stored.asks,
                   migrated=stored.migrated), True


def append_run(root, client_id: str, session_id: str, *, mode: str, specialist_ids: list[str],
               result: dict) -> CouncilRun:
    require_permission(root, client_id, "council")
    s = get_session(root, client_id, session_id)
    if s is None:
        raise StoreError(f"no such session: {client_id}/{session_id}")
    run = CouncilRun(index=len(s.runs) + 1, timestamp=datetime.now().isoformat(timespec="seconds"),
                     mode=mode, specialist_ids=list(specialist_ids), result=result)
    d = _legacy_or_dir(root, client_id, session_id)
    session_store.add_run(d, {"index": run.index, "timestamp": run.timestamp, "mode": mode,
                              "specialist_ids": list(specialist_ids), "result": result})
    session_store.rebuild_session_md(d, get_client(root, client_id).alias)
    return run


def purge_expired(root, *, today: _date) -> list[str]:
    deleted: list[str] = []
    for c in list_clients(root):
        days = c.consent.retain_days
        if days is None:
            continue
        for s in list_sessions(root, c.id):
            if s.status != "closed":
                continue
            if (today - _date.fromisoformat(s.date)) > timedelta(days=days):
                d = _legacy_or_dir(root, c.id, s.id)
                if d.is_dir():
                    shutil.rmtree(d)
                else:
                    _session_file(root, c.id, s.id).unlink(missing_ok=True)
                index_path = _client_dir(root, c.id) / "index.sqlite"
                if index_path.exists():
                    from melori_engine.practice.index import ClientIndex
                    ix = ClientIndex(root, c.id)
                    try:
                        ix.drop_session(s.id)
                    finally:
                        ix.close()
                deleted.append(f"{c.id}/{s.id}")
        cutoff = timedelta(days=days)
        for name in ("recap.json",):
            path = _client_dir(root, c.id) / name
            if path.exists():
                try:
                    created = datetime.fromisoformat(json.loads(path.read_text(encoding="utf-8")).get("created", ""))
                except (ValueError, TypeError, json.JSONDecodeError, OSError):
                    created = None
                if created is not None and datetime.combine(today, datetime.min.time()) - created > cutoff:
                    path.unlink()
                    deleted.append(f"{c.id}/{name}")
        chats = _client_dir(root, c.id) / "chats"
        for path in chats.glob("*.json") if chats.is_dir() else ():
            try:
                created = datetime.fromisoformat(json.loads(path.read_text(encoding="utf-8")).get("created", ""))
            except (ValueError, TypeError, json.JSONDecodeError, OSError):
                created = None
            if created is not None and datetime.combine(today, datetime.min.time()) - created > cutoff:
                path.unlink()
                deleted.append(f"{c.id}/{path.name}")
    return deleted
