param(
    [string]$OutputPath = "bin\nioh3_seed_accelerator.dll"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $PSScriptRoot "native_seed_accelerator.cu"
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
$nvcc = Join-Path $env:ProgramFiles "NVIDIA GPU Computing Toolkit\CUDA\v13.3\bin\nvcc.exe"
if (-not (Test-Path -LiteralPath $nvcc)) {
    throw "CUDA 13.3 nvcc.exe was not found."
}

$architectures = '-gencode=arch=compute_75,code=sm_75 -gencode=arch=compute_86,code=sm_86 -gencode=arch=compute_89,code=sm_89 -gencode=arch=compute_89,code=compute_89 -gencode=arch=compute_120,code=sm_120 -gencode=arch=compute_120,code=compute_120'
$command = 'call "{0}" >nul && "{1}" -O3 -shared {2} -o "{3}" "{4}"' -f $vcvars, $nvcc, $architectures, $resolvedOutput, $sourcePath
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $resolvedOutput)) {
    throw "The CUDA Seed accelerator build failed."
}

Get-Item -LiteralPath $resolvedOutput | Select-Object FullName, Length, LastWriteTime
Get-FileHash -LiteralPath $resolvedOutput -Algorithm SHA256
