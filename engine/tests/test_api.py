import json
import logging

ALL = ["transcript", "video", "council", "retain"]

def mk(client, alias="Анна", perms=ALL, retain_days=None):
    r = client.post("/api/clients", json={"alias": alias, "permissions": perms,
                                          "consent_date": "2026-09-27", "retain_days": retain_days})
    assert r.status_code == 200, r.text
    return r.json()

def sse_events(text):
    out = []
    for block in text.strip().split("\n\n"):
        lines = dict(l.split(": ", 1) for l in block.splitlines() if ": " in l)
        out.append((lines["event"], json.loads(lines["data"])))
    return out

def test_create_and_list(client):
    c = mk(client)
    assert c["id"] == "anna" and c["consent"]["active"] is True
    assert [x["id"] for x in client.get("/api/clients").json()] == ["anna"]


def test_search_words_uses_client_index(client):
    c = mk(client)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/transcript",
                json={"segments": [{"source": "others", "start_ms": 0, "end_ms": 1,
                                    "text": "Плохо сплю, сна почти нет"}]})
    result = client.get(f"/api/clients/{c['id']}/search", params={"q": 'сон" OR (x*', "mode": "words"})
    assert result.status_code == 200 and result.json()["results"][0]["session_id"] == s["id"]


def test_search_without_retain_does_not_create_index(client, settings):
    c = mk(client, perms=["transcript"])
    result = client.get(f"/api/clients/{c['id']}/search", params={"q": "anything", "mode": "words"})
    assert result.status_code == 200 and result.json()["results"] == []
    assert not (settings.clients_root / c["id"] / "index.sqlite").exists()


def test_builtin_templates_follow_the_requested_language(client):
    ru = client.get("/api/templates?language=ru-RU").json()
    en = client.get("/api/templates").json()
    assert ru[0]["sections"][0]["title"] == "Субъективно" and en[0]["sections"][0]["title"] == "Subjective"


def test_templates_list_create_and_builtin_is_read_only(client):
    listed = client.get("/api/templates")
    assert listed.status_code == 200
    assert [x["id"] for x in listed.json()][:3] == ["soap", "dap", "grow"]
    created = client.post("/api/templates", json={"name": "Фокус", "sections": [
        {"key": "focus", "title": "Фокус", "guidance": ""},
    ]})
    assert created.status_code == 201
    assert client.put("/api/templates/soap", json={"name": "X", "sections": [
        {"key": "x", "title": "X", "guidance": ""},
    ]}).status_code == 409


def test_client_template_roundtrip_and_force_delete(client):
    c = mk(client)
    custom = client.post("/api/templates", json={"name": "Фокус", "sections": [
        {"key": "focus", "title": "Фокус", "guidance": ""},
    ]}).json()
    assert client.put(f"/api/clients/{c['id']}", json={"template_id": "dap"}).json()["template_id"] == "dap"
    assert client.get(f"/api/clients/{c['id']}").json()["client"]["template_id"] == "dap"
    assert client.put(f"/api/clients/{c['id']}", json={"template_id": "missing"}).status_code == 422
    assert client.put(f"/api/clients/{c['id']}", json={"template_id": custom["id"]}).status_code == 200
    conflict = client.delete(f"/api/templates/{custom['id']}")
    assert conflict.status_code == 409 and conflict.json()["clients"] == 1
    assert client.delete(f"/api/templates/{custom['id']}?force=true").status_code == 204
    assert client.get(f"/api/clients/{c['id']}").json()["client"]["template_id"] == "soap"


def test_template_id_is_never_a_path(client, settings):
    c = mk(client)
    (settings.data_dir / "evil.json").write_text('{"id": "x", "name": "x", "sections": [{"key": "a", "title": "a"}]}', encoding="utf-8")
    assert client.put(f"/api/clients/{c['id']}", json={"template_id": "../evil"}).status_code == 422
    assert client.put("/api/templates/t-00000000", json={"name": "X", "sections": [
        {"key": "x", "title": "X", "guidance": ""},
    ]}).status_code == 404
    assert client.delete("/api/templates/t-00000000").status_code == 404

def test_open_session_403_without_transcript(client):
    c = mk(client, perms=["council"])
    assert client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).status_code == 403

def test_open_and_close_with_retain(client):
    c = mk(client)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    r = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": "N"})
    assert r.status_code == 200 and r.json()["retained"] is True

def test_close_without_retain_returns_not_retained(client):
    c = mk(client, perms=["transcript"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": "N"}).json() == {"retained": False, "session": None}

def test_revoke_then_close_is_403(client):
    c = mk(client); s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post(f"/api/clients/{c['id']}/revoke")
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": "N"}).status_code == 403

def test_delete_then_404(client):
    c = mk(client)
    assert client.delete(f"/api/clients/{c['id']}").status_code == 204
    assert client.get(f"/api/clients/{c['id']}").status_code == 404

def _retained_session(client, c):
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": "N"})
    return s

def test_delete_session_removes_only_that_session(client):
    c = mk(client)
    a = _retained_session(client, c)
    b = _retained_session(client, c)
    assert client.delete(f"/api/clients/{c['id']}/sessions/{a['id']}").status_code == 204
    left = [x["id"] for x in client.get(f"/api/clients/{c['id']}").json()["sessions"]]
    assert left == [b["id"]]

def test_delete_missing_session_is_404(client):
    c = mk(client)
    assert client.delete(f"/api/clients/{c['id']}/sessions/2026-01-01-01").status_code == 404

def test_delete_session_unsafe_id_is_4xx_not_500(client):
    c = mk(client)
    assert client.delete(f"/api/clients/{c['id']}/sessions/..%2F..%2Fclient").status_code in (404, 422)

def test_delete_session_allowed_after_revocation(client):
    # removing data is exactly what a client who revoked consent may ask for
    c = mk(client)
    s = _retained_session(client, c)
    client.post(f"/api/clients/{c['id']}/revoke")
    assert client.delete(f"/api/clients/{c['id']}/sessions/{s['id']}").status_code == 204

def test_unsafe_id_is_4xx_not_500(client):
    assert client.get("/api/clients/..%2Fx").status_code in (404, 422)

def test_council_stream_format_without_client(client):
    r = client.post("/api/psych-council/stream", json={"situation": "worry", "specialist_ids": ["cbt"]})
    assert [k for k, _ in sse_events(r.text)] == ["plan", "lens_start", "opinion", "lens_done",
                                                  "synthesis_start", "synthesis", "done"]

def test_council_specialists_are_listed(client):
    r = client.get("/api/psych-council/specialists")
    assert r.status_code == 200
    items = r.json()
    assert len(items) >= 10 and {"id", "name", "paradigm"} <= set(items[0])
    assert r.headers.get("x-council-concurrency")

def test_council_bound_client_without_council_perm_403(client):
    c = mk(client, perms=["transcript", "retain"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    r = client.post("/api/psych-council/stream", json={"situation": "x", "specialist_ids": ["cbt"], "client_id": c["id"], "session_id": s["id"]})
    assert r.status_code == 403

def test_council_bound_client_persists_run(client):
    c = mk(client); s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post("/api/psych-council/stream", json={"situation": "x", "specialist_ids": ["cbt"], "client_id": c["id"], "session_id": s["id"]})
    assert len(client.get(f"/api/clients/{c['id']}").json()["sessions"][0]["runs"]) == 1

def test_consent_template_and_signed_upload(client):
    c = mk(client); t = client.get(f"/api/clients/{c['id']}/consent/template").json()
    assert t["version"] == "v1" and "Анна" in t["text"]
    assert client.post(f"/api/clients/{c['id']}/consent/signed", files={"file": ("s.pdf", b"PDF")}).json()["consent"]["signed_sha256"]

def test_session_text_never_in_logs(client, caplog):
    caplog.set_level(logging.DEBUG); c = mk(client)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": "MARKER-91d2"})
    client.post("/api/psych-council/stream", json={"situation": "MARKER-91d2", "specialist_ids": ["cbt"]})
    assert "MARKER-91d2" not in caplog.text


def _ask_session(client, perms=ALL, alias="Анна"):
    c = mk(client, perms=perms, alias=alias)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    return c, s


def test_ask_needs_council(client):
    c, s = _ask_session(client, ["transcript", "retain"])
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/asks",
                       json={"question": "q", "segments": []}).status_code == 403


def test_ask_with_no_speech_does_not_call_the_model(client, llm):
    c, s = _ask_session(client)
    response = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/asks",
                           json={"question": "q", "segments": [], "elapsed_ms": 10})
    assert response.status_code == 200 and response.json()["ask"]["answer"] in ("пока ничего не сказано", "nothing has been said yet")
    assert llm.calls == []


def test_ask_is_kept_only_with_retain(client, settings):
    c, s = _ask_session(client)
    body = {"question": "q", "segments": [{"source": "others", "start_ms": 0, "end_ms": 1, "text": "hello"}], "elapsed_ms": 10}
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/asks", json=body).json()["stored"] is True
    path = settings.clients_root / c["id"] / "sessions" / s["id"] / "asks.jsonl"
    assert path.exists() and len(path.read_text(encoding="utf-8").splitlines()) == 1
    detail = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}").json()
    assert len(detail["asks"]) == 1
    c2, s2 = _ask_session(client, ["transcript", "council"], alias="Борис")
    r2 = client.post(f"/api/clients/{c2['id']}/sessions/{s2['id']}/asks", json=body)
    assert r2.json()["stored"] is False
    assert not (settings.clients_root / c2["id"] / "sessions" / s2["id"] / "asks.jsonl").exists()


def test_quick_ask_uses_the_preset_question(client, llm):
    c, s = _ask_session(client)
    r = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/asks",
                    json={"kind": "agreed", "question": "", "segments": [{"source": "others", "text": "привет"}]})
    assert r.status_code == 200 and "К чему мы договорились" in r.json()["ask"]["question"]
    assert "К чему мы договорились" in llm.calls[-1][-1]["content"]


def test_empty_ask_answers_return_502_after_three_attempts(client, llm):
    c, s = _ask_session(client)
    llm.reply = ""
    r = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/asks",
                    json={"question": "q", "segments": [{"source": "others", "text": "hello"}]})
    assert r.status_code == 502 and len(llm.calls) == 3


def test_note_warnings_are_returned_and_kept(client, llm):
    c, s = _ask_session(client)
    llm.reply = '{"data":"x","assessment":"F51.0","plan":"p"}'
    result = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                         json={"template_id": "dap", "segments": _note_segments()}).json()
    assert result["note"]["warnings"]
    assert client.get(f"/api/clients/{c['id']}/sessions/{s['id']}/notes/1").json()["warnings"]

def test_export_without_council_by_default(client):
    c = mk(client); s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post("/api/psych-council/stream", json={"situation": "x", "specialist_ids": ["cbt"], "client_id": c["id"], "session_id": s["id"]})
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": "BODY"})
    plain = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}/export").json()
    assert "BODY" in plain["markdown"] and "CONVERGENCES" not in plain["markdown"]
    full = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}/export", params={"include_council": "true"}).json()
    assert "CONVERGENCES" in full["markdown"]

def test_purge_report_exists(client):
    assert client.get("/api/purge-report").json() == {"deleted": []}

def test_council_request_language_reaches_the_lenses(client, llm):
    client.post("/api/psych-council/stream", json={"situation": "worry", "specialist_ids": ["cbt"], "language": "ru"})
    assert "Russian" in llm.calls[0][-1]["content"]


def _note_segments():
    return [{"source": "me", "start_ms": 0, "end_ms": 100, "text": "We discussed a goal."}]


def test_generate_without_retain_writes_nothing(client, settings, llm):
    c = mk(client, perms=["transcript", "council"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    llm.reply = '{"data":"goal","assessment":"clear","plan":"next step"}'
    r = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                    json={"template_id": "dap", "segments": _note_segments()})
    assert r.status_code == 200 and r.json()["stored"] is False
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/close", json={"note": ""}).json()["retained"] is False
    assert not (settings.clients_root / c["id"] / "sessions" / s["id"]).exists()


def test_generate_requires_council_consent(client):
    c = mk(client, perms=["transcript", "retain"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                       json={"template_id": "dap", "segments": _note_segments()}).status_code == 403


def test_practitioner_note_requires_retain(client):
    c = mk(client, perms=["transcript"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    r = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes",
                    json={"template_id": "dap", "fields": {"data": "d", "assessment": "a", "plan": "p"}, "parent": None})
    assert r.status_code == 403 and r.json()["detail"] == "nothing is retained"


def test_generate_and_practitioner_versions_are_kept(client, llm):
    c = mk(client)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    llm.reply = '{"data":"goal","assessment":"clear","plan":"next step"}'
    first = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                        json={"template_id": "dap", "segments": _note_segments()}).json()
    assert first["stored"] is True
    manual = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes",
                         json={"template_id": "dap", "fields": {"data": "edited", "assessment": "a", "plan": "p"}, "parent": 1})
    assert manual.status_code == 200 and manual.json()["n"] == 2 and manual.json()["parent"] == 1
    llm.reply = '{"data":"new","assessment":"new a","plan":"new p"}'
    third = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                        json={"template_id": "dap", "segments": _note_segments()}).json()
    assert third["note"]["n"] == 3
    assert client.get(f"/api/clients/{c['id']}/sessions/{s['id']}/notes/2").json()["fields"]["data"] == "edited"


def test_practitioner_edit_keeps_the_headings_of_its_parent(client, llm):
    c = mk(client)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    llm.reply = '{"data":"цель","assessment":"ясно","plan":"шаг"}'
    first = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                        json={"template_id": "dap", "language": "ru", "segments": _note_segments()}).json()
    edit = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes",
                       json={"template_id": "dap", "fields": {"data": "x", "assessment": "a", "plan": "p"}, "parent": 1}).json()
    assert edit["sections"] == first["note"]["sections"] and edit["sections"][0][1] == "Данные"
    orphan = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes",
                         json={"template_id": "dap", "fields": {"data": "x", "assessment": "a", "plan": "p"}, "parent": 9})
    assert orphan.status_code == 422


def test_session_detail_and_export_are_part_based(client, llm):
    c = mk(client); s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    llm.reply = '{"data":"goal","assessment":"clear","plan":"next step"}'
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/transcript",
                json={"segments": _note_segments(), "duration_ms": 100})
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                json={"template_id": "dap"})
    detail = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}").json()
    assert detail["note"]["fields"]["data"] == "goal" and len(detail["versions"]) == 1
    exported = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}/export").json()["markdown"]
    assert "### Data" in exported and "me: We discussed a goal." in exported


def _email_session(client, perms, llm):
    c = mk(client, perms=perms)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    llm.reply = '{"data":"goal","assessment":"clear","plan":"next step"}'
    generated = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/notes:generate",
                            json={"template_id": "dap", "segments": _note_segments()})
    assert generated.status_code == 200
    return c, s


def test_email_needs_council(client):
    c = mk(client, perms=["transcript", "retain"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/email:generate").status_code == 403


def test_email_needs_a_note(client):
    c = mk(client, perms=["transcript", "council", "retain"])
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/email:generate").status_code == 422


def test_email_is_written_from_the_note_only(client, llm, settings):
    c, s = _email_session(client, ALL, llm)
    llm.reply = '{"subject":"s","body":"b"}'
    r = client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/email:generate")
    assert r.status_code == 200 and r.json()["stored"] is True and r.json()["email"]["from_note"] == 1, r.text
    assert r.json()["email"]["provenance"]["input"] == "note"
    text = json.dumps(llm.calls[-1], ensure_ascii=False)
    assert "We discussed a goal." not in text and "next step" in text
    detail = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}").json()
    assert detail["email"]["subject"] == "s"
    assert "Письмо клиенту" in (settings.clients_root / c["id"] / "sessions" / s["id"] / "session.md").read_text(encoding="utf-8")


def test_email_without_retain_writes_nothing(client, llm, settings):
    c, s = _email_session(client, ["transcript", "council"], llm)
    llm.reply = '{"subject":"s","body":"b"}'
    base = f"/api/clients/{c['id']}/sessions/{s['id']}/email:generate"
    # nothing is stored and the engine keeps no copy: without the note in the body there is nothing to write from
    assert client.post(base).status_code == 422
    note = {"sections": [["plan", "План"]], "fields": {"plan": "Купить обувь."}}
    r = client.post(base, json={"note": note})
    assert r.status_code == 200 and r.json()["stored"] is False, r.text
    assert "Купить обувь." in llm.calls[-1][-1]["content"]
    assert client.post(base, json={"note": {"sections": [["plan", "План"]], "fields": {"x": "y"}}}).status_code == 422
    assert not (settings.clients_root / c["id"] / "sessions" / s["id"] / "email.json").exists()
    assert client.put(f"/api/clients/{c['id']}/sessions/{s['id']}/email", json={"subject":"x","body":"y"}).status_code == 403


def test_practitioner_email_edit(client, llm):
    c, s = _email_session(client, ALL, llm)
    llm.reply = '{"subject":"s","body":"b"}'
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/email:generate")
    edited = client.put(f"/api/clients/{c['id']}/sessions/{s['id']}/email", json={"subject":"s2","body":"b2"})
    assert edited.status_code == 200 and edited.json()["author"] == "practitioner"
    assert client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/email/opened").status_code == 204
    assert client.get(f"/api/clients/{c['id']}/sessions/{s['id']}").json()["email"]["opened_in_mail_at"] is not None


def test_session_detail_carries_provenance_and_email(client, llm):
    c, s = _email_session(client, ALL, llm)
    llm.reply = '{"subject":"s","body":"b"}'
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/email:generate")
    detail = client.get(f"/api/clients/{c['id']}/sessions/{s['id']}").json()
    assert detail["note"]["provenance"]["input"] == "transcript"
    assert detail["versions"][0]["provenance"]["chars_sent"] > 0
    assert detail["email"]["from_note"] == 1


def test_client_can_be_deleted_after_a_search(client, settings):
    c = mk(client)
    s = client.post(f"/api/clients/{c['id']}/sessions", json={"meeting_type": "session"}).json()
    client.post(f"/api/clients/{c['id']}/sessions/{s['id']}/transcript",
                json={"segments": [{"i": 1, "source": "others", "start_ms": 0, "end_ms": 900, "text": "про сон"}], "duration_ms": 900})
    assert client.get(f"/api/clients/{c['id']}/search", params={"q": "сна", "mode": "words"}).status_code == 200
    assert client.delete(f"/api/clients/{c['id']}").status_code == 204
    assert not (settings.clients_root / c["id"]).exists()
