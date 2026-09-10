param(
    [string]$Python = $env:NIOH3_BUILD_PYTHON,
    [string]$Output = (Join-Path 'deliverables/frontend-v2' ('portable-' + (Get-Date -Format 'yyyyMMdd-HHmmss')))
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (-not $Python) { $Python = Join-Path $projectRoot '.codex_tmp/v2-build-env/Scripts/python.exe' }
if (-not (Test-Path -LiteralPath $Python -PathType Leaf)) { throw 'Set NIOH3_BUILD_PYTHON to the isolated build environment Python.' }
$outputPath = if ([IO.Path]::IsPathFullyQualified($Output)) { [IO.Path]::GetFullPath($Output) } else { [IO.Path]::GetFullPath((Join-Path $projectRoot $Output)) }
if (Test-Path -LiteralPath $outputPath) { throw 'Output already exists; choose a new portable directory.' }
function Invoke-Checked([string]$Executable, [string[]]$Arguments) {
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Build command failed: $Executable (exit $LASTEXITCODE)" }
}
$env:PYTHONUTF8 = '1'
$env:NIOH3_BUILD_PYTHON = $Python
# The caller prepares this environment from packaging/requirements-v2.lock.txt.
Invoke-Checked $Python @('-m', 'pip', 'check')
Invoke-Checked 'npm.cmd' @('run', 'build')
$buildRoot = Join-Path $projectRoot ('.codex_tmp/v2-build-' + [guid]::NewGuid().ToString('N'))
$workers = Join-Path $buildRoot 'workers'
Invoke-Checked $Python @('tools/write_v2_python_manifest.py', $workers)
foreach ($role in @('search', 'protected')) {
    Invoke-Checked $Python @('-m', 'PyInstaller', '--clean', '--noconfirm', '--distpath', $workers,
        '--workpath', (Join-Path $buildRoot $role), (Join-Path $projectRoot "packaging/$role-worker.spec"))
}
Invoke-Checked 'node.exe' @('tools/package_frontend_v2.mjs', $outputPath, $workers)
Invoke-Checked 'node.exe' @('tools/verify_frontend_v2.mjs', $outputPath)
