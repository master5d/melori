"""Dump REAL engine responses into src/consult/__fixtures__/ so the UI's api.ts is tested against the
engine's actual contract, not an imagined one. Re-run after any API change:

    engine/.venv/Scripts/python engine/tests/dump_contract_fixtures.py
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from fastapi.testclient import TestClient

from melori_engine.app import create_app
from melori_engine.config import Settings
from melori_engine.consent_template import render_consent
from melori_engine.practice.models import Client, Consent

OUT = Path(__file__).resolve().parents[2] / "src" / "consult" / "__fixtures__"
TOKEN = "t" * 32


class FakeLLM:
    def complete(self, messages, response_format=None):
        if "follow-up email" in messages[0]["content"]:
            return '{"subject":"Следующая встреча","body":"Договорились о следующем шаге."}'
        if response_format is not None:
            return '{"data":"Session data.","assessment":"Working formulation.","plan":"Next step."}'
        return "Opinion text.\nCONVERGENCES:\n- shared view\nDIVERGENCES:\n- differing view"


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    data = Path(tempfile.mkdtemp())
    s = Settings(token=TOKEN, port=0, data_dir=data, llm_base_url="http://127.0.0.1:11434/v1",
                 llm_model="local", corpus_dir=None)
    fx: dict[str, object] = {}
    preview_forms = [
        {"alias": "Анна", "permissions": ["transcript", "video", "council", "retain"], "date": "2026-09-27", "retain_days": 30},
        {"alias": "Борис", "permissions": ["transcript"], "date": "2026-09-28", "retain_days": None},
        {"alias": "Вера", "permissions": ["transcript", "retain"], "date": "2026-09-29", "retain_days": None},
    ]
    fx["consent_preview"] = [
        {"form": {"alias": item["alias"], "permissions": {permission: permission in item["permissions"] for permission in ("transcript", "video", "council", "retain")}, "retainDays": item["retain_days"], "consentDate": item["date"]}, "date": item["date"], "text": render_consent(Client(id="preview", alias=item["alias"], tags=[], created="", intake="", consent=Consent(permissions=frozenset(item["permissions"]), date=item["date"], retain_days=item["retain_days"])),)}
        for item in preview_forms
    ]
    with TestClient(create_app(s, llm=FakeLLM())) as c:
        c.headers.update({"X-Melori-Token": TOKEN})
        fx["health"] = c.get("/health").json()
        fx["templates"] = c.get("/api/templates").json()
        fx["create_client"] = c.post("/api/clients", json={
            "alias": "Анна", "permissions": ["transcript", "council", "retain"],
            "consent_date": "2026-09-27", "retain_days": 90}).json()
        cid = fx["create_client"]["id"]
        sess = c.post(f"/api/clients/{cid}/sessions", json={"meeting_type": "session"}).json()
        c.post(f"/api/clients/{cid}/sessions/{sess['id']}/transcript", json={
            "segments": [{"source": "me", "start_ms": 0, "end_ms": 100, "text": "A session fact."}],
            "duration_ms": 100,
        })
        generated = c.post(f"/api/clients/{cid}/sessions/{sess['id']}/notes:generate", json={
            "template_id": "dap",
        }).json()
        fx["note_version"] = generated["note"]
        fx["ask"] = c.post(f"/api/clients/{cid}/sessions/{sess['id']}/asks", json={
            "question": "What was discussed?", "segments": [{"source": "me", "start_ms": 0, "end_ms": 100, "text": "A session fact."}], "elapsed_ms": 100,
        }).json()
        fx["email_draft"] = c.post(f"/api/clients/{cid}/sessions/{sess['id']}/email:generate").json()["email"]
        fx["session_detail"] = c.get(f"/api/clients/{cid}/sessions/{sess['id']}").json()
        c.post("/api/psych-council/stream", json={"situation": "x", "specialist_ids": ["cbt"],
                                                  "client_id": cid, "session_id": sess["id"]})
        fx["close_session"] = c.post(f"/api/clients/{cid}/sessions/{sess['id']}/close",
                                     json={"note": "# Session\n\nText."}).json()
        fx["search"] = c.get(f"/api/clients/{cid}/search", params={"q": "session"}).json()
        fx["chat"] = c.post(f"/api/clients/{cid}/chats", json={"question": "What was discussed?", "language": "en"}).json()
        fx["recap"] = c.post(f"/api/clients/{cid}/recap", json={"language": "en"}).json()
        fx["list_clients"] = c.get("/api/clients").json()
        fx["client_detail"] = c.get(f"/api/clients/{cid}").json()
        fx["consent_template"] = c.get(f"/api/clients/{cid}/consent/template").json()
        fx["export_plain"] = c.get(f"/api/clients/{cid}/sessions/{sess['id']}/export").json()
        fx["export_with_council"] = c.get(f"/api/clients/{cid}/sessions/{sess['id']}/export",
                                          params={"include_council": "true"}).json()
        fx["revoke"] = c.post(f"/api/clients/{cid}/revoke").json()
        fx["purge_report"] = c.get("/api/purge-report").json()
        fx["error_403"] = c.post(f"/api/clients/{cid}/sessions", json={"meeting_type": "session"}).json()
    for name, value in fx.items():
        (OUT / f"{name}.json").write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n",
                                          encoding="utf-8", newline="\n")
    print(f"wrote {len(fx)} fixtures to {OUT}")


if __name__ == "__main__":
    main()
