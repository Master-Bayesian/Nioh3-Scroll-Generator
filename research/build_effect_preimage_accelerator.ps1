param(
    [string]$OutputPath = "bin\nioh3_effect_preimage_accelerator.dll"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $PSScriptRoot "native_effect_preimage_accelerator.cpp"
$resolvedOutput = Join-Path $projectRoot $OutputPath
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw "Visual Studio Installer vswhere.exe was not found."
}
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) {
    throw "A Visual C++ x64 toolchain was not found."
}
$vcvars = Join-Path $visualStudio "VC\Auxiliary\Build\vcvars64.bat"
$command = 'call "{0}" >nul && cl.exe /nologo /std:c++20 /O2 /EHsc /LD /Fe:"{1}" "{2}" d3d11.lib dxgi.lib d3dcompiler.lib' -f $vcvars, $resolvedOutput, $sourcePath
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $resolvedOutput)) {
    throw "The Direct3D 11 effect preimage accelerator build failed."
}

Get-Item -LiteralPath $resolvedOutput | Select-Object FullName, Length, LastWriteTime
Get-FileHash -LiteralPath $resolvedOutput -Algorithm SHA256
