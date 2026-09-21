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
# The build root and the one shared Cargo target come from the same resolver the
# migration gates and tools/run_python_tests.ps1 use
# (tests/migration/cargo_target.py): an explicit NIOH3_BUILD_ROOT or
# CARGO_TARGET_DIR wins, otherwise a local Windows host builds on the D:
# delivery volume and a CI host keeps its platform temp root. Nothing this build
# writes may land in the checkout or the C: system temp, so every call below,
# the host and launcher included, inherits this one external target instead of
# its own workspace `target/` directory.
$resolvePaths=@'
import sys
sys.path.insert(0, sys.argv[1])
import cargo_target
print(str(cargo_target.temp_root()))
print(str(cargo_target.cargo_target_dir('tauri-target')))
'@
$resolvedPaths=@(& $Python -c $resolvePaths (Join-Path $PWD 'tests/migration'))
if($LASTEXITCODE -ne 0 -or $resolvedPaths.Count -lt 2){
    throw 'Failed to resolve the project build root and Cargo target'
}
$buildTempRoot=$resolvedPaths[0].Trim()
$env:CARGO_TARGET_DIR=$resolvedPaths[1].Trim()
$env:NIOH3_PYTHON=$Python
$env:PYTHONUTF8='1'
Checked 'npm.cmd' @('run','typecheck')
Checked 'node.exe' @('apps/tauri/build.mjs')
Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','apps/launcher/Cargo.toml')
Checked 'cargo.exe' @('build','--release','--locked','--manifest-path','apps/tauri/src-tauri/Cargo.toml')
# Staging lives under the resolved build root, never inside the checkout. A
# failed build keeps its own staging directory for diagnosis; a successful one
# removes exactly that directory and nothing else.
$stagingName='tauri-build-'+[guid]::NewGuid().ToString('N')
$build=Join-Path $buildTempRoot $stagingName
$workers=Join-Path $build 'workers'
$succeeded=$false
try {
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
    $succeeded=$true
} finally {
    if(-not $succeeded) {
        Write-Host "Build failed; the staging directory is kept for diagnosis: $build"
    } elseif(Test-Path -LiteralPath $build -PathType Container) {
        $resolvedRoot=[System.IO.Path]::GetFullPath($buildTempRoot)
        $resolvedBuild=[System.IO.Path]::GetFullPath($build)
        $resolvedPrefix=$resolvedRoot.TrimEnd([char[]]@('\','/'))+[System.IO.Path]::DirectorySeparatorChar
        # Remove only this run's own staging directory, and only after proving
        # the resolved path is that exact child of the resolved build temp root.
        if($resolvedBuild.StartsWith($resolvedPrefix,[System.StringComparison]::OrdinalIgnoreCase) -and
            (Split-Path -Leaf $resolvedBuild) -eq $stagingName) {
            Remove-Item -LiteralPath $resolvedBuild -Recurse -Force
            Write-Host "Removed this run's build staging: $resolvedBuild"
        } else {
            Write-Host "Kept build staging outside the expected root: $resolvedBuild"
        }
    }
}
