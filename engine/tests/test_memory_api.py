"""Subproject D over HTTP: chat and recap consent, client isolation, retention (written by the controller at
acceptance — the delegate's packet left these plan tests out)."""
import json
from datetime import date, datetime, timedelta

from melori_engine.practice import store as st
from tests.test_api import ALL, mk


def closed_session(client, llm, c, words, plan="Шаг на неделю"):
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    base = f"/api/clients/{c['id']}/sessions/{s['id']}"
    client.post(base + "/transcript", json={"segments": [
        {"i": 1, "source": "others", "start_ms": 0, "end_ms": 900, "text": words}], "duration_ms": 900})
    llm.reply = json.dumps({"data": words, "assessment": "Оценка", "plan": plan}, ensure_ascii=False)
    assert client.post(base + "/notes:generate", json={"template_id": "dap"}).status_code == 200
    client.post(base + "/close", json={"note": ""})
    return s


def test_chat_needs_council(client):
    c = mk(client, perms=["transcript", "retain"])
    assert client.post(f"/api/clients/{c['id']}/chats", json={"question": "что было?"}).status_code == 403


def test_chat_sends_only_this_clients_fragments(client, llm):
    a = mk(client, alias="Анна")
    b = mk(client, alias="Борис")
    closed_session(client, llm, a, "Мучает бессонница уже месяц")
    closed_session(client, llm, b, "Думаю про налоги и декларацию")
    llm.reply = "Бессонница упоминалась [x]."
    llm.calls.clear()
    r = client.post(f"/api/clients/{a['id']}/chats", json={"question": "что с бессонницей?"})
    assert r.status_code == 200, r.text
    sent = json.dumps(llm.calls[-1], ensure_ascii=False)
    assert "бессонница" in sent and "налоги" not in sent and "декларац" not in sent


def test_chat_without_findings_does_not_call_the_model(client, llm):
    a = mk(client)
    closed_session(client, llm, a, "Мучает бессонница уже месяц")
    llm.calls.clear()
    r = client.post(f"/api/clients/{a['id']}/chats", json={"question": "что с налогами?"}).json()
    assert llm.calls == [] and r["turn"]["a"] == "в записях этого нет"


def test_chat_is_kept_only_with_retain(client, llm, settings):
    kept = mk(client, alias="Анна")
    closed_session(client, llm, kept, "Мучает бессонница уже месяц")
    llm.reply = "Ответ."
    first = client.post(f"/api/clients/{kept['id']}/chats", json={"question": "бессонница?"}).json()
    assert first["stored"] is True and first["chat"] == 1
    again = client.post(f"/api/clients/{kept['id']}/chats", json={"question": "а ещё бессонница?", "chat": 1}).json()
    assert again["chat"] == 1
    assert len(client.get(f"/api/clients/{kept['id']}/chats/1").json()["turns"]) == 2
    loose = mk(client, alias="Вера", perms=["transcript", "council"])
    r = client.post(f"/api/clients/{loose['id']}/chats", json={"question": "бессонница?"}).json()
    assert r["stored"] is False and r["chat"] is None
    assert not (settings.clients_root / loose["id"] / "chats").exists()


def test_recap_without_a_closed_session_does_not_call_the_model(client, llm):
    c = mk(client)
    llm.calls.clear()
    assert client.post(f"/api/clients/{c['id']}/recap").status_code == 422
    assert llm.calls == []
    assert client.get(f"/api/clients/{c['id']}/recap").json()["quick"] is None


def test_recap_quick_and_model_and_staleness(client, llm, settings):
    c = mk(client)
    closed_session(client, llm, c, "Мучает бессонница уже месяц", plan="Ложиться до полуночи")
    quick = client.get(f"/api/clients/{c['id']}/recap").json()["quick"]
    assert quick["plan"]["text"] == "Ложиться до полуночи" and quick["days_ago"] == 0
    llm.reply = '{"points": ["Договорились ложиться до полуночи", "Проверить сон", "Тема работы"]}'
    made = client.post(f"/api/clients/{c['id']}/recap").json()  # the card posts no body
    assert made["stored"] is True and made["recap"]["provenance"]["language"] == "ru"
    assert made["recap"]["provenance"]["chars_sent"] > 0
    assert (settings.clients_root / c["id"] / "recap.json").exists()
    assert client.get(f"/api/clients/{c['id']}/recap").json()["model_stale"] is False
    closed_session(client, llm, c, "Сон стал лучше")
    assert client.get(f"/api/clients/{c['id']}/recap").json()["model_stale"] is True


def test_purge_removes_expired_chats_and_recap(client, llm, settings):
    c = mk(client, retain_days=30)
    root = settings.clients_root
    old = (datetime.now() - timedelta(days=40)).isoformat(timespec="seconds")
    fresh = datetime.now().isoformat(timespec="seconds")
    (root / c["id"] / "chats").mkdir(parents=True)
    (root / c["id"] / "chats" / "001.json").write_text(json.dumps({"n": 1, "created": old, "turns": []}), encoding="utf-8")
    (root / c["id"] / "chats" / "002.json").write_text(json.dumps({"n": 2, "created": fresh, "turns": []}), encoding="utf-8")
    (root / c["id"] / "recap.json").write_text(json.dumps({"created": old, "sessions": [], "points": []}), encoding="utf-8")
    st.purge_expired(root, today=date.today())
    assert not (root / c["id"] / "chats" / "001.json").exists()
    assert (root / c["id"] / "chats" / "002.json").exists()
    assert not (root / c["id"] / "recap.json").exists()
