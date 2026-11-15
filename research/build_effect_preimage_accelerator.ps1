param(
    [string]$OutputPath = "bin\nioh3_effect_preimage_accelerator.dll"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $PSScriptRoot "native_effect_preimage_accelerator.cpp"
$resolvedOutput = if ([System.IO.Path]::IsPathRooted($OutputPath)) { $OutputPath } else { Join-Path $projectRoot $OutputPath }
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$buildRoot = if ($env:NIOH3_BUILD_ROOT) { $env:NIOH3_BUILD_ROOT } else { 'D:\Nioh3_v080_deliverables' }
$shaderBuildDirectory = Join-Path $buildRoot 'tmp/native-effect-preimage'
$env:TEMP = $shaderBuildDirectory
$env:TMP = $shaderBuildDirectory
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
$seedShader = Join-Path $PSScriptRoot 'native_seed_search.hlsl'
$entries = @(
    @('CollectNatural','seed_collect_natural'),
    @('CollectR4','seed_collect_r4'),
    @('CollectAuxiliary','seed_collect_auxiliary'),
    @('FilterPrimary','seed_filter_primary'),
    @('FilterAuxiliary','seed_filter_auxiliary'),
    @('SeedSelfTest','seed_self_test')
)
$commands = [System.Collections.Generic.List[string]]::new()
$commands.Add(('call "{0}" >nul' -f $vcvars))
$commands.Add(('fxc.exe /nologo /T cs_5_0 /E main /O3 /Gis /Fh:"{0}" /Vn g_preimage_shader_bytecode "{1}"' -f $preimageHeader,$preimageHlsl))
$commands.Add(('fxc.exe /nologo /T cs_5_0 /E main /O3 /Gis /Fh:"{0}" /Vn g_effect_filter_shader_bytecode "{1}"' -f $effectFilterHeader,$effectFilterHlsl))
foreach ($entry in $entries) {
    $header = Join-Path $shaderBuildDirectory ($entry[1] + '.h')
    $commands.Add(('fxc.exe /nologo /T cs_5_0 /E {0} /O3 /Gis /Fh:"{1}" /Vn g_{2} "{3}"' -f $entry[0],$header,$entry[1],$seedShader))
}
$commands.Add(('cl.exe /nologo /std:c++20 /O2 /MT /EHsc /LD /I"{0}" /Fe:"{1}" "{2}" d3d11.lib dxgi.lib /link /IMPLIB:"{3}"' -f $shaderBuildDirectory,$resolvedOutput,$sourcePath,(Join-Path $shaderBuildDirectory 'nioh3_effect_preimage_accelerator.lib')))
Push-Location -LiteralPath $shaderBuildDirectory
try { & cmd.exe /d /s /c ($commands -join ' && ') } finally { Pop-Location }
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $resolvedOutput)) {
    throw "The Direct3D 11 effect and seed accelerator build failed."
}
$sourceFiles = @('research/native_effect_preimage_accelerator.cpp','research/native_directcompute_seed.inl','research/native_seed_search.hlsl')
$sources = [ordered]@{}
foreach ($relative in $sourceFiles) { $sources[$relative] = (Get-FileHash -LiteralPath (Join-Path $projectRoot $relative) -Algorithm SHA256).Hash.ToLowerInvariant() }
[ordered]@{
    schema = 'nioh3-directcompute-build/v1'
    seed_search_abi = 1
    sources = $sources
    binary_sha256 = (Get-FileHash -LiteralPath $resolvedOutput -Algorithm SHA256).Hash.ToLowerInvariant()
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath ([System.IO.Path]::ChangeExtension($resolvedOutput,'.build.json')) -Encoding utf8NoBOM

Get-Item -LiteralPath $resolvedOutput | Select-Object FullName, Length, LastWriteTime
Get-FileHash -LiteralPath $resolvedOutput -Algorithm SHA256
