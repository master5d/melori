$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$engine = Join-Path $root 'engine'
$py = (Get-Command python -ErrorAction Stop).Source
& $py -c "import sys; sys.exit(0 if sys.version_info >= (3,11) else 1)"
if ($LASTEXITCODE -ne 0) { throw 'Python >= 3.11 required' }
& $py -m venv (Join-Path $engine '.venv')
if ($LASTEXITCODE -ne 0) { throw 'venv failed' }
& (Join-Path $engine '.venv\Scripts\python.exe') -m pip install -e $engine
if ($LASTEXITCODE -ne 0) { throw 'pip install failed' }
Write-Output "engine ready: $(Join-Path $engine '.venv\Scripts\python.exe')"
