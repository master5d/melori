import json
from datetime import date

from melori_engine.practice import recap


class LLM:
    def complete(self, messages):
        return '```json\n{"points": ["one", "two", "three"]}\n```'


def test_model_recap_accepts_fenced_json_and_limits_points():
    points, attempts = recap.model_recap(LLM(), [("2026-09-12", "Plan: rest")], "en")
    assert points == ["one", "two", "three"]
    assert attempts == 1


def test_model_recap_accepts_markdown_list():
    class Markdown:
        def complete(self, messages):
            return "- one\n- two\n- three"
    assert recap.model_recap(Markdown(), [("2026-09-12", "x")], "en")[0] == ["one", "two", "three"]


def test_recap_stale_when_closed_session_is_not_recorded():
    class Session:
        def __init__(self, sid):
            self.id, self.status = sid, "closed"
    assert recap.recap_stale({"sessions": ["old"]}, [Session("new")])
    assert not recap.recap_stale({"sessions": ["new"]}, [Session("new")])


def _closed(tmp_path, template_id, sections, fields, date_="2026-09-01", subject=None):
    from melori_engine.practice import store as st, sessions as S
    from melori_engine.practice.models import Consent
    c = st.get_client(tmp_path, "anna") or st.create_client(tmp_path, "Anna", tags=[], consent=Consent(
        permissions=frozenset({"transcript", "council", "retain"}), date="2026-09-01", retain_days=None))
    s = st.open_session(tmp_path, c.id, date=date_, meeting_type="session")
    d = S.session_dir(tmp_path, c.id, s.id)
    S.add_note(d, author="model", template_id=template_id, fields=fields, sections=sections, parent=None)
    if subject:
        S.write_email(d, {"subject": subject, "body": "b", "author": "model", "from_note": 1, "created": "",
                          "provenance": None, "opened_in_mail_at": None})
    st.close_session(tmp_path, c.id, s.id, note="")
    return c


def test_quick_recap_takes_will_for_grow_and_the_email_subject(tmp_path):
    c = _closed(tmp_path, "grow", (("goal", "Goal"), ("will", "Will"), ("reality", "Reality")),
                {"goal": "g", "will": "walk daily", "reality": "r"}, subject="After our session")
    q = recap.quick_recap(tmp_path, c.id, today=date(2026, 9, 13))
    assert q["plan"] == {"title": "Will", "text": "walk daily"} and q["email_subject"] == "After our session"
    assert q["days_ago"] == 12


def test_quick_recap_falls_back_to_the_last_section(tmp_path):
    c = _closed(tmp_path, "t-00000001", (("focus", "Focus"), ("next", "Next")), {"focus": "f", "next": "n"})
    assert recap.quick_recap(tmp_path, c.id, today=date(2026, 9, 1))["plan"] == {"title": "Next", "text": "n"}


def test_model_recap_takes_the_one_list_under_another_key_and_sets_the_language():
    # local-floor answered {"summary": [...]} in English when neither the key nor the language was asked (2026-09-29)
    class L:
        def __init__(self): self.calls = []
        def complete(self, messages):
            self.calls.append(messages)
            return '{"summary": ["один", "два", "три"]}'
    llm = L()
    points, _ = recap.model_recap(llm, [("2026-09-01", "План: гулять")], "ru")
    assert points == ["один", "два", "три"] and "Russian" in llm.calls[0][-1]["content"]


def test_model_recap_without_points_fails_loudly():
    import pytest
    class Empty:
        def complete(self, messages): return "no list here"
    with pytest.raises(recap.RecapError):
        recap.model_recap(Empty(), [("2026-09-01", "x")], "en")
