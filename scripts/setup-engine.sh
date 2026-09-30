#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
engine="$root/engine"
py="$(command -v python3)"
"$py" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)' || {
  echo 'Python >= 3.11 required' >&2
  exit 1
}
"$py" -m venv "$engine/.venv"
"$engine/.venv/bin/python" -m pip install -e "$engine"
echo "engine ready: $engine/.venv/bin/python"
