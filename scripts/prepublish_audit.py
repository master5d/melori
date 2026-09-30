#!/usr/bin/env python3
"""Pre-publication audit for melori. Fail-closed: a check that cannot run BLOCKS publication.

Exit codes: 0 clean · 1 findings · 2 at least one check unavailable."""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Literal

Status = Literal["clean", "findings", "unavailable"]
JUNK_PREFIXES = ("logs/", "dist/", "training/", "magenta-phase0-bench.sh")


@dataclass
class CheckResult:
    name: str
    status: Status
    findings: list[str] = field(default_factory=list)


def _git(repo: Path, *args: str) -> str:
    out = subprocess.run(["git", "-C", str(repo), *args], check=True,
                         capture_output=True, text=True, encoding="utf-8", errors="replace")
    return out.stdout


def load_patterns(path: Path) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    return [ln.strip() for ln in lines if ln.strip() and not ln.lstrip().startswith("#")]


def patterns_from(files: list[Path]) -> list[str]:
    """Merge patterns from every existing file; a missing file is skipped (local file is optional)."""
    out: list[str] = []
    for f in files:
        if f.exists():
            out += load_patterns(f)
    return out


def check_history_secrets(repo: Path) -> CheckResult:
    exe = shutil.which("gitleaks")
    if exe is None:
        return CheckResult("history-secrets", "unavailable", ["gitleaks not on PATH"])
    proc = subprocess.run([exe, "git", str(repo), "--no-banner", "--redact",
                           "--report-format", "json", "--report-path", "-"],
                          capture_output=True, text=True, encoding="utf-8", errors="replace")
    if proc.returncode not in (0, 1):
        return CheckResult("history-secrets", "unavailable", [proc.stderr.strip()[:500]])
    try:
        items = json.loads(proc.stdout or "[]")
    except json.JSONDecodeError:
        return CheckResult("history-secrets", "unavailable", ["unparseable gitleaks output"])
    findings = [f"{i.get('RuleID')} {i.get('File')} @ {str(i.get('Commit'))[:10]}" for i in items]
    return CheckResult("history-secrets", "findings" if findings else "clean", findings)


def check_internal_leaks(repo: Path, patterns: list[str]) -> CheckResult:
    findings: list[str] = []
    for pat in patterns:
        re.compile(pat)  # invalid pattern must raise, not silently match nothing
        proc = subprocess.run(["git", "-C", str(repo), "log", "--all", "-p", "--format=%h",
                               "-G", pat], capture_output=True, text=True,
                              encoding="utf-8", errors="replace")
        if proc.returncode != 0:
            return CheckResult("internal-leaks", "unavailable", [proc.stderr.strip()[:500]])
        rx = re.compile(pat)
        commit = "?"
        for line in proc.stdout.splitlines():
            if re.fullmatch(r"[0-9a-f]{7,40}", line.strip()):
                commit = line.strip()
            elif line.startswith(("+", "-")) and not line.startswith(("+++", "---")):
                m = rx.search(line)
                if m:
                    findings.append(f"{commit}: {m.group(0)}")
    findings = sorted(set(findings))
    return CheckResult("internal-leaks", "findings" if findings else "clean", findings)


def check_licenses(repo: Path) -> CheckResult:
    findings: list[str] = []
    cargo = shutil.which("cargo")
    npx = shutil.which("npx")
    if cargo is None or npx is None:
        return CheckResult("licenses", "unavailable", ["cargo or npx not on PATH"])
    deny = subprocess.run([cargo, "deny", "--manifest-path", str(repo / "src-tauri" / "Cargo.toml"),
                           "check", "licenses"], capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    if "no such command" in (deny.stderr or "") or "no such subcommand" in (deny.stderr or ""):
        return CheckResult("licenses", "unavailable", ["cargo-deny not installed"])
    if deny.returncode != 0:
        findings.append("cargo-deny: " + deny.stderr.strip()[-800:])
    lc = subprocess.run([npx, "--yes", "license-checker", "--production", "--summary",
                         "--failOn", "GPL;AGPL;LGPL;SSPL;CC-BY-NC"], cwd=str(repo),
                        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if lc.returncode != 0:
        findings.append("license-checker: " + (lc.stderr or lc.stdout).strip()[-800:])
    return CheckResult("licenses", "findings" if findings else "clean", findings)


def check_tracked_junk(repo: Path) -> CheckResult:
    files = _git(repo, "ls-files").splitlines()
    findings = [f for f in files if f.startswith(JUNK_PREFIXES)]
    return CheckResult("tracked-junk", "findings" if findings else "clean", findings)


def run_all(repo: Path, patterns: list[str]) -> list[CheckResult]:
    return [check_history_secrets(repo), check_internal_leaks(repo, patterns),
            check_licenses(repo), check_tracked_junk(repo)]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=str(Path(__file__).resolve().parents[1]))
    ap.add_argument("--patterns", action="append", default=None,
                    help="patterns file (repeatable); default: shipped + .superpowers local file")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    repo = Path(a.repo)
    files = [Path(p) for p in a.patterns] if a.patterns else [
        Path(__file__).with_name("prepublish_patterns.txt"),
        repo / ".superpowers" / "prepublish_patterns.local.txt"]
    patterns = patterns_from(files)
    results = run_all(repo, patterns)
    if a.json:
        print(json.dumps([asdict(r) for r in results], ensure_ascii=False, indent=2))
    else:
        for r in results:
            print(f"[{r.status.upper():11}] {r.name} ({len(r.findings)})")
            for f in r.findings[:50]:
                print(f"    {f}")
    if any(r.status == "unavailable" for r in results):
        return 2
    return 1 if any(r.status == "findings" for r in results) else 0


if __name__ == "__main__":
    sys.exit(main())
