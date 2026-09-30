import threading

from melori_engine.practice import sessions as S, store as st
from melori_engine.practice.models import Consent, Segment


def mk(root, perms=("transcript", "council", "retain")):
    return st.create_client(root, "Анна", tags=[], consent=Consent(
        permissions=frozenset(perms), date="2026-09-28", retain_days=None))


def test_open_creates_a_session_directory(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    assert (d / "meta.json").is_file() and S.read_meta(d)["schema"] == 2


def test_versions_never_overwrite_each_other(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    a = S.add_note(d, author="model", template_id="soap", fields={"plan": "a"},
                   sections=(("plan", "Plan"),), parent=None)
    b = S.add_note(d, author="practitioner", template_id="soap", fields={"plan": "b"},
                   sections=(("plan", "Plan"),), parent=a.n)
    assert (a.n, b.n) == (1, 2) and S.read_note(d, 1).fields == {"plan": "a"} and S.read_note(d, 2).parent == 1


def test_concurrent_versions_get_distinct_numbers(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    got = []

    def w(i):
        got.append(S.add_note(d, author="model", template_id="soap", fields={"plan": str(i)},
                              sections=(("plan", "Plan"),), parent=None).n)

    ts = [threading.Thread(target=w, args=(i,)) for i in range(8)]
    [t.start() for t in ts]
    [t.join() for t in ts]
    assert sorted(got) == list(range(1, 9))


def test_close_without_retain_leaves_nothing(tmp_path):
    c = mk(tmp_path, perms=("transcript",))
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    st.close_session(tmp_path, c.id, s.id, note="x")
    assert not S.session_dir(tmp_path, c.id, s.id).exists()


def test_session_md_is_rebuilt_from_parts(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    S.write_transcript(d, [Segment(1, "others", 0, 900, "Спала плохо")])
    S.add_note(d, author="model", template_id="soap", fields={"plan": "режим сна"},
               sections=(("plan", "План"),), parent=None)
    S.rebuild_session_md(d, "Анна")
    md = (d / "session.md").read_text(encoding="utf-8")
    assert "Анна" in md and "План" in md and "режим сна" in md and "Спала плохо" in md


def test_note_readable_after_its_template_is_deleted(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    S.add_note(d, author="model", template_id="t-gone", fields={"focus": "x"},
               sections=(("focus", "Фокус"),), parent=None)
    assert S.read_note(d, 1).sections == (("focus", "Фокус"),)


def test_delete_and_purge_remove_the_directory(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    st.delete_session(tmp_path, c.id, s.id)
    assert not S.session_dir(tmp_path, c.id, s.id).exists()


def test_session_note_is_the_current_note_only(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-28", meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    S.write_transcript(d, [Segment(1, "others", 0, 900, "Спала плохо")])
    S.add_note(d, author="model", template_id="soap", fields={"plan": "режим сна"}, sections=(("plan", "План"),), parent=None)
    note = st.get_session(tmp_path, c.id, s.id).note
    assert note == "## План\n\nрежим сна"
    assert "Спала плохо" not in note and c.alias not in note


def test_closing_an_unmigrated_session_without_retain_removes_its_file(tmp_path):
    c = mk(tmp_path, perms=("transcript",))
    p = tmp_path / c.id / "sessions" / "2026-09-28-01.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("not a v1 header", encoding="utf-8")
    st.close_session(tmp_path, c.id, "2026-09-28-01", note="")
    assert not p.exists()
