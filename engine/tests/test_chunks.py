from melori_engine.practice import store as st, sessions as S, chunks as C
from melori_engine.practice.models import Consent, Segment


def mk(root):
    return st.create_client(root, "Анна", tags=[], consent=Consent(permissions=frozenset({"transcript", "council", "retain"}), date="2026-09-01", retain_days=None))


def session_with(root, cid, date, lines, plan="шаг"):
    s = st.open_session(root, cid, date=date, meeting_type="session")
    d = S.session_dir(root, cid, s.id)
    S.write_transcript(d, [Segment(i, src, i * 1000, i * 1000 + 900, t) for i, (src, t) in enumerate(lines, 1)])
    S.add_note(d, author="model", template_id="dap", fields={"data": "старое", "plan": "x"}, sections=(("data", "Данные"), ("plan", "План")), parent=None)
    S.add_note(d, author="practitioner", template_id="dap", fields={"data": "данные", "plan": plan}, sections=(("data", "Данные"), ("plan", "План")), parent=1)
    return s, d


def test_windows_overlap_and_only_the_current_note(tmp_path):
    c = mk(tmp_path)
    lines = [("others" if i % 2 else "me", "фраза " * 30) for i in range(10)]
    _, d = session_with(tmp_path, c.id, "2026-09-01", lines)
    ch = C.chunks_for_session(d)
    tr = [x for x in ch if x.kind == "transcript"]
    assert len(tr) >= 3 and all(len(x.text) <= 600 + 200 for x in tr)
    assert tr[0].text.splitlines()[-1] == tr[1].text.splitlines()[0]
    assert tr[0].text.startswith(("Я: ", "Клиент: "))
    notes = [x for x in ch if x.kind == "note"]
    assert [(x.label, x.text) for x in notes] == [("Данные", "данные"), ("План", "шаг")]


def test_mark_segments_are_not_indexed(tmp_path):
    c = mk(tmp_path)
    _, d = session_with(tmp_path, c.id, "2026-09-01", [("mark", "")])
    assert not [x for x in C.chunks_for_session(d) if x.kind == "transcript"]
