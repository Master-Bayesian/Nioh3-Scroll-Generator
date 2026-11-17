#Requires -Version 7
param([Parameter(Mandatory)][string]$Evidence)
$ErrorActionPreference='Stop'
$PSNativeCommandUseErrorActionPreference=$false
if(-not $env:NIOH3_PYTHON){throw 'NIOH3_PYTHON must name the prepared Python interpreter'}
New-Item -ItemType Directory -Force -Path $Evidence | Out-Null
& $env:NIOH3_PYTHON --version | Out-Null
function Checked([string]$Program,[string[]]$Arguments) {
    & $Program @Arguments
    if($LASTEXITCODE -ne 0){throw "Native packaging gate failed: $Program ($LASTEXITCODE)"}
}
Checked $env:NIOH3_PYTHON @('tools/verify_native_build_manifest.py')
Checked 'cargo' @('fetch','--locked','--manifest-path','crates/nioh3-worker/Cargo.toml')
Checked 'cargo' @('fetch','--locked','--manifest-path','crates/nioh3-protected/Cargo.toml')
Checked 'cargo' @('build','--locked','--offline','--manifest-path','crates/nioh3-worker/Cargo.toml','--bin','nioh3-readonly-worker')
Checked 'cargo' @('build','--locked','--offline','--manifest-path','crates/nioh3-protected/Cargo.toml','--bin','nioh3-protected-worker')
& ./tools/run_python_tests.ps1 -Python $env:NIOH3_PYTHON -TestPath tools/tests -PytestArgument @('-q')
if($LASTEXITCODE -ne 0){throw 'Python packaging suite failed'}
$worker=Join-Path $env:CARGO_TARGET_DIR 'debug/nioh3-readonly-worker.exe'
$selector=Join-Path $Evidence 'selector'
& ./tools/run_python_tests.ps1 -Python $env:NIOH3_PYTHON -ScriptPath tools/verify_selector_wraith_worker_e2e.py -ScriptArgument @('--worker',$worker,'--output',$selector)
if($LASTEXITCODE -ne 0){throw 'Selector-state worker E2E failed'}
$env:NIOH3_RUST_SEARCH_WORKER=$worker
$env:NIOH3_RUST_PROTECTED_WORKER=Join-Path $env:CARGO_TARGET_DIR 'debug/nioh3-protected-worker.exe'
$env:NIOH3_RUST_SEARCH_GAME_FILE_VERSION='2.0.2.0'
$env:NIOH3_RUST_PROTECTED_GAME_FILE_VERSION='2.0.2.0'
New-Item -ItemType Directory -Force -Path apps/tauri/dist | Out-Null
Checked 'cargo' @('fetch','--locked','--manifest-path','apps/tauri/src-tauri/Cargo.toml')
Checked 'cargo' @('test','--locked','--offline','--manifest-path','apps/tauri/src-tauri/Cargo.toml','--','--ignored','--skip','dump_role_launch_for_acceptance')
[ordered]@{pass=$true;scope='Owned packaging, selector and broker fixtures; no game/user-save dispatch';selector_report=(Join-Path $selector 'report.json');broker_cases=4} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Evidence 'packaging.json') -Encoding utf8NoBOM
