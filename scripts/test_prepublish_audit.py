import subprocess
from pathlib import Path

import pytest

import prepublish_audit as pa


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    r = tmp_path / "r"
    r.mkdir()
    _git(r, "init", "-q")
    _git(r, "config", "user.email", "t@example.com")
    _git(r, "config", "user.name", "t")
    (r / "a.txt").write_text("hello\n", encoding="utf-8")
    _git(r, "add", "a.txt")
    _git(r, "commit", "-qm", "init")
    return r


def test_internal_leak_found_in_old_commit_even_if_deleted_now(repo: Path) -> None:
    # assembled at runtime so this file does not itself trip the audit it tests
    leak = "host node.example." + "ts" + ".net\n"
    (repo / "b.txt").write_text(leak, encoding="utf-8")
    _git(repo, "add", "b.txt"); _git(repo, "commit", "-qm", "leak")
    _git(repo, "rm", "-q", "b.txt"); _git(repo, "commit", "-qm", "remove")
    res = pa.check_internal_leaks(repo, [r"\.ts\.net\b"])
    assert res.status == "findings"
    assert any("ts.net" in f for f in res.findings)


def test_internal_leak_clean_twin(repo: Path) -> None:
    res = pa.check_internal_leaks(repo, [r"\.ts\.net\b"])
    assert res.status == "clean" and res.findings == []


def test_tracked_junk_flags_logs_dir(repo: Path) -> None:
    (repo / "logs").mkdir()
    (repo / "logs" / "desops.log").write_text("x\n", encoding="utf-8")
    _git(repo, "add", "logs/desops.log"); _git(repo, "commit", "-qm", "log")
    res = pa.check_tracked_junk(repo)
    assert res.status == "findings"
    assert "logs/desops.log" in res.findings


def test_tracked_junk_clean_twin(repo: Path) -> None:
    assert pa.check_tracked_junk(repo).status == "clean"


def test_missing_gitleaks_is_unavailable_not_clean(repo: Path, monkeypatch) -> None:
    monkeypatch.setattr(pa.shutil, "which", lambda name: None)
    res = pa.check_history_secrets(repo)
    assert res.status == "unavailable"


def test_exit_code_unavailable_blocks(monkeypatch, repo: Path) -> None:
    monkeypatch.setattr(pa, "run_all", lambda repo, patterns: [
        pa.CheckResult("x", "clean", []), pa.CheckResult("y", "unavailable", [])])
    assert pa.main(["--repo", str(repo)]) == 2


def test_local_patterns_file_is_merged(tmp_path: Path) -> None:
    shipped = tmp_path / "shipped.txt"
    local = tmp_path / "local.txt"
    shipped.write_text("# comment\n\\.ts\\.net\\b\n", encoding="utf-8")
    local.write_text("secret-host\n", encoding="utf-8")
    pats = pa.patterns_from([shipped, local, tmp_path / "missing.txt"])
    assert "secret-host" in pats and any("ts" in p for p in pats)
