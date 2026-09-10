param([string]$Python = $env:NIOH3_PYTHON)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $Python) {
    $localPython = Join-Path $projectRoot '.venv/Scripts/python.exe'
    if (Test-Path -LiteralPath $localPython) { $Python = $localPython }
    else {
        $pythonCommand = Get-Command python -ErrorAction SilentlyContinue
        if ($pythonCommand) { $Python = $pythonCommand.Source }
    }
}
if (-not $Python) { throw 'Set NIOH3_PYTHON or pass -Python with a Python 3.12+ executable.' }
Push-Location $projectRoot
try {
    $env:NIOH3_PYTHON = $Python
    $env:NIOH3_REVIEW_UI = '1'
    & $Python -c 'import jsonschema, cryptography, pefile'
    if ($LASTEXITCODE -ne 0) { throw 'Install requirements.txt into the selected Python environment.' }
    if (-not (Test-Path -LiteralPath 'node_modules')) { throw 'Run npm ci first.' }
    npm run desktop
    if ($LASTEXITCODE -ne 0) { throw 'Desktop launch failed.' }
}
finally { Pop-Location }
