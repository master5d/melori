"""Is the disk holding client data encrypted? on / off / unknown. Never raises."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Literal


def _probe(path: Path) -> bool | None:
    try:
        if sys.platform == "win32":
            drive = Path(path).resolve().drive or "C:"
            ps = ("(New-Object -ComObject Shell.Application).NameSpace('%s').Self"
                  ".ExtendedProperty('System.Volume.BitLockerProtection')" % (drive + "\\"))
            out = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True,
                                 text=True, timeout=10).stdout.strip()
            if out in ("1", "3", "5"):
                return True
            if out in ("0", "2"):
                return False
            return None
        if sys.platform == "darwin":
            out = subprocess.run(["fdesetup", "status"], capture_output=True, text=True,
                                 timeout=10).stdout
            if "FileVault is On" in out:
                return True
            if "FileVault is Off" in out:
                return False
            return None
        out = subprocess.run(["lsblk", "-o", "TYPE"], capture_output=True, text=True,
                             timeout=10).stdout
        return True if "crypt" in out.split() else None
    except Exception:
        return None


def disk_encryption_status(path: Path) -> Literal["on", "off", "unknown"]:
    r = _probe(path)
    return "unknown" if r is None else ("on" if r else "off")
