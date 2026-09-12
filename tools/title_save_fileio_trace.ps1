[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Start', 'Stop', 'Status')]
    [string]$Action,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$')]
    [string]$RunId,

    [string]$OutputRoot = '.codex_tmp\title-save-fileio'
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$resolvedOutputRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot $OutputRoot))
$expectedParent = [System.IO.Path]::GetFullPath((Join-Path $projectRoot '.codex_tmp'))
if (-not $resolvedOutputRoot.StartsWith($expectedParent + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'The trace output root must remain under the project .codex_tmp directory.'
}

$runDirectory = Join-Path $resolvedOutputRoot $RunId
$tracePath = Join-Path $runDirectory 'FileIO.etl'
$metadataPath = Join-Path $runDirectory 'trace-metadata.json'
$wpr = Join-Path $env:SystemRoot 'System32\wpr.exe'

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdministrator)) {
    throw 'Windows FileIO tracing requires an Administrator PowerShell session for Start, Stop, and Status. A non-elevated WPR status query can incorrectly report that no recording is active.'
}

if ($Action -eq 'Status') {
    & $wpr -status
    exit $LASTEXITCODE
}

if ($Action -eq 'Start') {
    if (Test-Path -LiteralPath $tracePath) {
        throw "Refusing to overwrite the existing trace: $tracePath"
    }
    $status = (& $wpr -status 2>&1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "Could not query WPR status: $status"
    }
    if ($status -notmatch 'not recording') {
        throw 'WPR is already recording. Stop or preserve the existing recording first.'
    }
    New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
    & $wpr -start FileIO -filemode
    if ($LASTEXITCODE -ne 0) {
        throw "WPR failed to start with exit code $LASTEXITCODE"
    }
    [ordered]@{
        schema = 'nioh3-title-save-fileio-trace/v1'
        run_id = $RunId
        started_at_utc = [DateTime]::UtcNow.ToString('o')
        profile = 'FileIO'
        mode = 'file'
        trace_path = $tracePath
        stopped_at_utc = $null
    } | ConvertTo-Json | Set-Content -LiteralPath $metadataPath -Encoding utf8
    Write-Output "WPR FileIO recording started for $RunId"
    Write-Output $tracePath
    exit 0
}

if (-not (Test-Path -LiteralPath $runDirectory -PathType Container)) {
    throw "Unknown trace run: $runDirectory"
}
if (Test-Path -LiteralPath $tracePath) {
    throw "Refusing to overwrite the existing trace: $tracePath"
}
& $wpr -stop $tracePath "Nioh 3 title-save ownership $RunId" -skipPdbGen
if ($LASTEXITCODE -ne 0) {
    throw "WPR failed to stop with exit code $LASTEXITCODE"
}
$metadata = Get-Content -LiteralPath $metadataPath -Raw -Encoding utf8 | ConvertFrom-Json
$metadata.stopped_at_utc = [DateTime]::UtcNow.ToString('o')
$metadata | ConvertTo-Json | Set-Content -LiteralPath $metadataPath -Encoding utf8
Write-Output "WPR FileIO recording stopped for $RunId"
Write-Output $tracePath
