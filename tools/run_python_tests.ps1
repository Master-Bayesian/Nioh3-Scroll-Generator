[CmdletBinding()]
param(
    [string]$Python,
    [string]$LuaLibrary,
    [string[]]$TestPath = @('tests'),
    [string[]]$PytestArgument = @('-q')
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

function Resolve-ProjectPython {
    param([string]$RequestedPython)

    $candidates = [System.Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($RequestedPython)) {
        $candidates.Add($RequestedPython)
    } elseif (-not [string]::IsNullOrWhiteSpace($env:NIOH3_PYTHON)) {
        $candidates.Add($env:NIOH3_PYTHON)
    } else {
        $candidates.Add((Join-Path $projectRoot '.codex_tmp/v2-build-env/Scripts/python.exe'))
        $candidates.Add((Join-Path $projectRoot '.venv/Scripts/python.exe'))
    }

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    throw @"
No prepared project Python environment was found.
Create .codex_tmp/v2-build-env and install requirements-dev.txt, or pass
-Python / NIOH3_PYTHON with an environment containing the tracked dependencies.
"@
}

$pythonExecutable = Resolve-ProjectPython -RequestedPython $Python
& $pythonExecutable -c 'import pytest, lupa'
if ($LASTEXITCODE -ne 0) {
    throw "The selected Python environment is incomplete: $pythonExecutable. Install requirements-dev.txt."
}

if ([string]::IsNullOrWhiteSpace($LuaLibrary) -and
    [string]::IsNullOrWhiteSpace($env:LUA54_LIBRARY)) {
    $ceLuaLibrary = 'C:\Program Files\Cheat Engine\lua53-64.dll'
    if (Test-Path -LiteralPath $ceLuaLibrary -PathType Leaf) {
        $LuaLibrary = $ceLuaLibrary
    }
}

if (-not [string]::IsNullOrWhiteSpace($LuaLibrary)) {
    if (-not (Test-Path -LiteralPath $LuaLibrary -PathType Leaf)) {
        throw "The requested Lua shared library does not exist: $LuaLibrary"
    }
    $env:LUA54_LIBRARY = (Resolve-Path -LiteralPath $LuaLibrary).Path
}

$pytestArguments = @('-m', 'pytest') + $PytestArgument + $TestPath
Write-Host "Project Python: $pythonExecutable"
if (-not [string]::IsNullOrWhiteSpace($env:LUA54_LIBRARY)) {
    Write-Host "Lua test library: $env:LUA54_LIBRARY"
}

& $pythonExecutable @pytestArguments
exit $LASTEXITCODE
