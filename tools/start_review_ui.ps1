param([string]$Python = $env:NIOH3_PYTHON)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $Python) {
    $candidates = @((Join-Path $projectRoot '.venv/Scripts/python.exe'), (Join-Path $env:USERPROFILE '.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'))
    $Python = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $Python) { throw 'Set NIOH3_PYTHON to a Python environment with requirements.txt installed.' }
$node = (Get-Command node -ErrorAction Stop).Source
Push-Location $projectRoot
try {
    $env:NIOH3_PYTHON = $Python
    $env:NIOH3_REVIEW_UI = '1'
    & $node apps/desktop/build.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Review UI build failed.' }
    $entry = Join-Path $projectRoot 'apps/desktop/dist/main.cjs'
    $electron = Join-Path $projectRoot 'node_modules/electron/dist/electron.exe'
    Start-Process -FilePath $electron -ArgumentList ('"' + $entry + '"') -WorkingDirectory $projectRoot
}
finally { Pop-Location }
