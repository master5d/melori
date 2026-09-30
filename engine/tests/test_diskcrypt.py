from melori_engine import diskcrypt


def test_unknown_when_probe_fails(monkeypatch, tmp_path):
    monkeypatch.setattr(diskcrypt, "_probe", lambda path: None)
    assert diskcrypt.disk_encryption_status(tmp_path) == "unknown"


def test_on_and_off(monkeypatch, tmp_path):
    monkeypatch.setattr(diskcrypt, "_probe", lambda path: True)
    assert diskcrypt.disk_encryption_status(tmp_path) == "on"
    monkeypatch.setattr(diskcrypt, "_probe", lambda path: False)
    assert diskcrypt.disk_encryption_status(tmp_path) == "off"
