param(
    [string]$OutputPath = "bin\nioh3_effect_preimage_accelerator.dll"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $PSScriptRoot "native_effect_preimage_accelerator.cpp"
$resolvedOutput = Join-Path $projectRoot $OutputPath
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$shaderBuildDirectory = Join-Path $projectRoot ".build\native_effect_preimage"
New-Item -ItemType Directory -Path $shaderBuildDirectory -Force | Out-Null

$sourceText = [System.IO.File]::ReadAllText($sourcePath)
function Export-EmbeddedShader {
    param(
        [string]$Symbol,
        [string]$OutputPath
    )
    $pattern = 'constexpr char ' + [regex]::Escape($Symbol) + '\[\] = R"HLSL\((.*?)\)HLSL";'
    $match = [regex]::Match(
        $sourceText,
        $pattern,
        [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    if (-not $match.Success) {
        throw "Embedded shader $Symbol was not found in $sourcePath."
    }
    $shaderSource = [regex]::Replace(
        $match.Groups[1].Value,
        '\)HLSL"\s*R"HLSL\(',
        ''
    )
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $shaderSource,
        [System.Text.UTF8Encoding]::new($false)
    )
}

$preimageHlsl = Join-Path $shaderBuildDirectory "native_preimage_shader.hlsl"
$effectFilterHlsl = Join-Path $shaderBuildDirectory "native_effect_filter_shader.hlsl"
$preimageHeader = Join-Path $shaderBuildDirectory "native_preimage_shader.h"
$effectFilterHeader = Join-Path $shaderBuildDirectory "native_effect_filter_shader.h"
Export-EmbeddedShader -Symbol "SHADER_SOURCE" -OutputPath $preimageHlsl
Export-EmbeddedShader -Symbol "EFFECT_FILTER_SHADER_SOURCE" -OutputPath $effectFilterHlsl

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw "Visual Studio Installer vswhere.exe was not found."
}
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) {
    throw "A Visual C++ x64 toolchain was not found."
}
$vcvars = Join-Path $visualStudio "VC\Auxiliary\Build\vcvars64.bat"
$command = 'call "{0}" >nul && fxc.exe /nologo /T cs_5_0 /E main /O3 /Gis /Fh:"{1}" /Vn g_preimage_shader_bytecode "{2}" && fxc.exe /nologo /T cs_5_0 /E main /O3 /Gis /Fh:"{3}" /Vn g_effect_filter_shader_bytecode "{4}" && cl.exe /nologo /std:c++20 /O2 /EHsc /LD /I"{5}" /Fe:"{6}" "{7}" d3d11.lib dxgi.lib' -f $vcvars, $preimageHeader, $preimageHlsl, $effectFilterHeader, $effectFilterHlsl, $shaderBuildDirectory, $resolvedOutput, $sourcePath
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $resolvedOutput)) {
    throw "The Direct3D 11 effect preimage accelerator build failed."
}

Get-Item -LiteralPath $resolvedOutput | Select-Object FullName, Length, LastWriteTime
Get-FileHash -LiteralPath $resolvedOutput -Algorithm SHA256
