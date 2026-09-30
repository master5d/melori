import logging

import pytest
from fastapi.testclient import TestClient

from melori_engine.app import create_app
from melori_engine.config import ConfigError, Settings


def test_health_with_token(client):
    r = client.get("/health")
    assert r.status_code == 200
    body = r.json()
    assert body["ok"] is True and body["version"] == "0.1.0"
    assert body["disk_encryption"] in {"on", "off", "unknown"}
    assert body["llm_local"] is True


def test_health_reports_remote_llm(tmp_path, llm):
    s = Settings(token="t" * 32, port=0, data_dir=tmp_path, llm_base_url="https://gw.example.net/v1",
                 llm_model="m", corpus_dir=None)
    with TestClient(create_app(s, llm=llm)) as c:
        assert c.get("/health", headers={"X-Melori-Token": "t" * 32}).json()["llm_local"] is False


def test_no_token_refused(settings, llm):
    with TestClient(create_app(settings, llm=llm)) as c:
        assert c.get("/health").status_code == 401


def test_wrong_token_refused(settings, llm):
    with TestClient(create_app(settings, llm=llm)) as c:
        assert c.get("/health", headers={"X-Melori-Token": "x" * 32}).status_code == 401


def test_empty_token_env_fails_closed(monkeypatch):
    monkeypatch.setenv("MELORI_ENGINE_TOKEN", "")
    with pytest.raises(ConfigError):
        Settings.from_env()


def test_data_dir_env_override(monkeypatch, tmp_path):
    monkeypatch.setenv("MELORI_ENGINE_TOKEN", "t" * 32)
    monkeypatch.setenv("MELORI_DATA_DIR", str(tmp_path / "d"))
    assert Settings.from_env().data_dir == tmp_path / "d"


def test_request_log_has_no_body(client, caplog):
    caplog.set_level(logging.INFO, logger="melori.engine")
    client.post("/health-echo-probe", json={"marker": "SECRET-MARKER-7f3a"})
    assert "SECRET-MARKER-7f3a" not in caplog.text
