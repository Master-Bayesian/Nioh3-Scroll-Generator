param([Parameter(Mandatory)][string]$Python,[Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
function Checked([string]$Program,[string[]]$Arguments) {
    & $Program @Arguments
    if($LASTEXITCODE -ne 0){throw "Command failed: $Program ($LASTEXITCODE)"}
}
$env:NIOH3_PYTHON=$Python
$env:PYTHONUTF8='1'
Checked 'npm.cmd' @('run','typecheck')
Checked 'node.exe' @('apps/tauri/build.mjs')
Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','apps/launcher/Cargo.toml')
Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','apps/tauri/src-tauri/Cargo.toml')
$build=Join-Path $PWD ('.codex_tmp/tauri-build-'+[guid]::NewGuid().ToString('N'))
$workers=Join-Path $build 'workers'
Checked $Python @('tools/write_v2_python_manifest.py',$workers)
foreach($role in @('search','protected')) {
    Checked $Python @('-m','PyInstaller','--clean','--noconfirm','--distpath',$workers,'--workpath',(Join-Path $build $role),"packaging/$role-worker.spec")
}
Checked $Python @('tools/package_tauri.py',$Output,$workers)
