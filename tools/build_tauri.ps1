param(
    [Parameter(Mandatory)][string]$Python,
    [Parameter(Mandatory)][string]$Output,
    # The default packaged graph is the Rust worker (owner-authorized local
    # backend switch). `-WorkerBackend python` remains available for
    # development, parity and the legacy Tk path, but it is not the shipped
    # graph. Either selection refuses to mix both backends.
    [ValidateSet('python','rust')][string]$WorkerBackend = 'rust'
)
$ErrorActionPreference='Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
function Checked([string]$Program,[string[]]$Arguments) {
    & $Program @Arguments
    if($LASTEXITCODE -ne 0){throw "Command failed: $Program ($LASTEXITCODE)"}
}
# One Cargo target directory for the whole build. An explicit CARGO_TARGET_DIR
# always wins (the packaging tools resolve the built EXEs from the same place);
# otherwise the default lives under the platform temp directory so a candidate
# build never fills the checkout volume. Every call below inherits it, the host
# and launcher included, instead of only the worker crates.
if([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
    $env:CARGO_TARGET_DIR = Join-Path ([System.IO.Path]::GetTempPath()) 'nioh3-tauri-target'
}
$env:NIOH3_PYTHON=$Python
$env:PYTHONUTF8='1'
Checked 'npm.cmd' @('run','typecheck')
Checked 'node.exe' @('apps/tauri/build.mjs')
Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','apps/launcher/Cargo.toml')
Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','apps/tauri/src-tauri/Cargo.toml')
$build=Join-Path $PWD ('.codex_tmp/tauri-build-'+[guid]::NewGuid().ToString('N'))
$workers=Join-Path $build 'workers'
if($WorkerBackend -eq 'rust') {
    Write-Host 'Rust worker backend: the shipped packaged graph'
    Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','crates/nioh3-worker/Cargo.toml','--bin','nioh3-readonly-worker')
    Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','crates/nioh3-protected/Cargo.toml','--bin','nioh3-protected-worker')
    Checked $Python @('tools/stage_rust_workers.py','--binaries',$env:CARGO_TARGET_DIR,'--workers',$workers)
    Checked $Python @('tools/package_tauri.py',$Output,$workers,'--worker-backend','rust')
} else {
    Write-Host 'Legacy python worker backend: development, parity and oracle use only'
    Checked $Python @('tools/write_v2_python_manifest.py',$workers)
    foreach($role in @('search','protected')) {
        Checked $Python @('-m','PyInstaller','--clean','--noconfirm','--distpath',$workers,'--workpath',(Join-Path $build $role),"packaging/$role-worker.spec")
    }
    Checked $Python @('tools/package_tauri.py',$Output,$workers)
}
