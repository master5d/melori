import json
from pathlib import Path

from melori_engine.practice import migrate, sessions as S, store as st
from melori_engine.practice.models import Consent


def mk(root):
    return st.create_client(root, "Анна", tags=[], consent=Consent(
        permissions=frozenset({"transcript", "council", "retain"}), date="2026-09-28", retain_days=None))


def v1(sid, body, runs=""):
    return f"---\nid: {sid}\nclient: Анна\ndate: 2026-09-28\ncreated: 2026-09-28T10:00:00\nstatus: closed\nmeeting_type: session\n{runs}---\n{body}"


def put(root, cid, sid, text):
    d = root / cid / "sessions"
    d.mkdir(parents=True, exist_ok=True)
    p = d / f"{sid}.md"
    p.write_bytes(text.encode("utf-8"))
    return p


def test_migrate_soap_and_transcript_preserves_source(tmp_path):
    c = mk(tmp_path); sid = "2026-09-28-01"
    raw = v1(sid, "# Transcript\nme: привет\nothers: ответ\n\n# SOAP\n## Subjective\nS\n## Objective\nO\n## Assessment\nA\n## Plan\nP\n")
    p = put(tmp_path, c.id, sid, raw)
    migrate.migrate_client(tmp_path, c.id)
    d = S.session_dir(tmp_path, c.id, sid)
    assert [x.source for x in S.read_transcript(d)] == ["me", "others"]
    assert S.read_note(d, 1).author == "import" and S.read_note(d, 1).template_id == "soap"
    assert len(S.read_note(d, 1).fields) == 4
    assert (d / "legacy.md").read_bytes() == raw.encode() and not p.exists()
    assert S.read_meta(d)["migrated_from"] == "v1"


def test_migrate_free_text(tmp_path):
    c = mk(tmp_path); sid = "2026-09-28-01"
    put(tmp_path, c.id, sid, v1(sid, "Ручная заметка без заголовков"))
    migrate.migrate_client(tmp_path, c.id)
    assert S.read_note(S.session_dir(tmp_path, c.id, sid), 1).fields == {"text": "Ручная заметка без заголовков"}


def test_migrate_runs(tmp_path):
    c = mk(tmp_path); sid = "2026-09-28-01"
    runs = 'runs:\n  - index: 1\n    timestamp: t1\n    mode: m\n    specialist_ids: []\n    result: {a: 1}\n  - index: 2\n    timestamp: t2\n    mode: m\n    specialist_ids: []\n    result: {b: 2}\n'
    put(tmp_path, c.id, sid, v1(sid, "text", runs))
    migrate.migrate_client(tmp_path, c.id)
    assert [r.index for r in S.read_runs(S.session_dir(tmp_path, c.id, sid))] == [1, 2]


def test_migrate_is_idempotent(tmp_path):
    c = mk(tmp_path); sid = "2026-09-28-01"
    put(tmp_path, c.id, sid, v1(sid, "text"))
    first = migrate.migrate_client(tmp_path, c.id)
    mtime = (S.session_dir(tmp_path, c.id, sid) / "legacy.md").stat().st_mtime_ns
    second = migrate.migrate_client(tmp_path, c.id)
    assert first == {"migrated": 1, "failed": []} and second == {"migrated": 0, "failed": []}
    assert (S.session_dir(tmp_path, c.id, sid) / "legacy.md").stat().st_mtime_ns == mtime


def test_migrate_one_bad_file_does_not_block_other_sessions(tmp_path):
    c = mk(tmp_path)
    bad = put(tmp_path, c.id, "2026-09-28-01", "not markdown")
    good = put(tmp_path, c.id, "2026-09-28-02", v1("2026-09-28-02", "good"))
    result = migrate.migrate_client(tmp_path, c.id)
    assert result == {"migrated": 1, "failed": ["2026-09-28-01"]} and bad.exists() and not good.exists()
    assert st.get_session(tmp_path, c.id, "2026-09-28-01").status == "unmigrated"


def test_a_failed_last_step_leaves_the_source_in_place(tmp_path, monkeypatch):
    c = mk(tmp_path); sid = "2026-09-28-01"
    raw = v1(sid, "text")
    p = put(tmp_path, c.id, sid, raw)
    def boom(*a, **k):
        raise OSError("disk full")
    monkeypatch.setattr(S, "rebuild_session_md", boom)
    assert migrate.migrate_client(tmp_path, c.id)["failed"] == [sid]
    assert p.read_bytes() == raw.encode() and not S.session_dir(tmp_path, c.id, sid).exists()


def test_soap_stops_at_the_next_top_level_section_and_uses_the_alias(tmp_path):
    c = mk(tmp_path); sid = "2026-09-28-01"
    put(tmp_path, c.id, sid, v1(sid, "# SOAP\n## Plan\nP\n\n# Council\nlong synthesis\n"))
    migrate.migrate_client(tmp_path, c.id)
    d = S.session_dir(tmp_path, c.id, sid)
    assert S.read_note(d, 1).fields["plan"] == "P"
    assert S.read_meta(d)["client_alias"] == c.alias
