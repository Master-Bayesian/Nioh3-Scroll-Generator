[CmdletBinding()]
param(
    [string]$BuildRoot = $env:NIOH3_BUILD_ROOT,
    [string]$CryptoSourceRoot,
    [ValidateSet('all', 'counter', 'counter-wrap', 'header', 'system-body', 'user-body')]
    [string]$TestCase = 'all',
    [switch]$Sanitize,
    [switch]$BuildHelper,
    [string]$LogPrefix = 'legacy-crypto'
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -lt 7) { throw 'PowerShell 7 is required.' }
$projectRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($BuildRoot)) { $BuildRoot = 'D:\Nioh3_v080_deliverables' }
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
if ([string]::IsNullOrWhiteSpace($CryptoSourceRoot)) {
    $CryptoSourceRoot = Join-Path $projectRoot 'third_party\nioh_savefile_decrypt'
}
$CryptoSourceRoot = [IO.Path]::GetFullPath($CryptoSourceRoot)
if ($LogPrefix -notmatch '^legacy-crypto[-a-zA-Z0-9_]*$') { throw 'Use a unique legacy-crypto log prefix.' }
$runRoot = Join-Path $BuildRoot ('build-cache\' + $LogPrefix)
$logs = Join-Path $BuildRoot 'deliverables\codex-release-candidate-20261002\logs'
$tempRoot = Join-Path $runRoot 'tmp'
New-Item -ItemType Directory -Path $runRoot,$logs,$tempRoot -Force | Out-Null
$savedTemp = $env:TEMP
$savedTmp = $env:TMP
$savedAsan = $env:ASAN_OPTIONS
try {
    $env:TEMP = $tempRoot
    $env:TMP = $tempRoot
    $vswhere = Join-Path ([Environment]::GetEnvironmentVariable('ProgramFiles(x86)')) 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Existing Visual Studio installer discovery tool is required.' }
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($installation)) { throw 'Existing MSVC x64 toolchain is required.' }
    & (Join-Path $installation 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) { throw 'MSVC developer environment did not expose cl.exe.' }
    $compileLog = Join-Path $logs ($LogPrefix + '-build.log')
    $base = @('/nologo', '/c', '/O2', '/MD', '/W3', '/Y-', '/utf-8', ('/I' + $CryptoSourceRoot), ('/Fd' + (Join-Path $runRoot 'compiler.pdb')))
    if ($Sanitize) { $base += @('/fsanitize=address', '/Zi') }
    function Build-Object([string]$Source, [string]$Name, [switch]$CSource) {
        $object = Join-Path $runRoot ($Name + '.obj')
        $options = $base + @('/Fo' + $object)
        if (-not $CSource) { $options += @('/EHsc', '/std:c++17') }
        & cl.exe @options $Source 2>&1 | Tee-Object -FilePath $compileLog -Append | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "MSVC compilation failed: $Source (exit $LASTEXITCODE)." }
        return $object
    }
    function Link-Executable([string]$Output, [string[]]$Objects) {
        $options = @('/nologo', '/MD')
        if ($Sanitize) { $options += @('/fsanitize=address', '/Zi') }
        $linkPdb = Join-Path $runRoot ([IO.Path]::GetFileNameWithoutExtension($Output) + '.pdb')
        & cl.exe @options @Objects ('/Fe' + $Output) /link /DEBUG /INCREMENTAL:NO ('/PDB:' + $linkPdb) 2>&1 |
            Tee-Object -FilePath $compileLog -Append | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "MSVC link failed: $Output (exit $LASTEXITCODE)." }
    }
    "SOURCE=$CryptoSourceRoot; ASAN=$Sanitize; COMPILER=$((Get-Command cl.exe).Source)" |
        Set-Content -LiteralPath $compileLog
    $aes = Build-Object (Join-Path $CryptoSourceRoot 'aes.c') 'aes' -CSource

    $test = Build-Object (Join-Path $projectRoot 'tests\legacy_crypto_bounds.cpp') 'legacy_crypto_bounds'
    $testExe = Join-Path $runRoot 'legacy_crypto_bounds.exe'
    Link-Executable $testExe @($aes,$test)
    if ($BuildHelper) {
        $crypto = Build-Object (Join-Path $CryptoSourceRoot 'CryptoState.cpp') 'CryptoState'
        $editor = Build-Object (Join-Path $CryptoSourceRoot 'SaveEditor.cpp') 'SaveEditor'
        $main = Build-Object (Join-Path $CryptoSourceRoot 'NiohSavefiledecrypt.cpp') 'NiohSavefiledecrypt'
        $helper = Join-Path $runRoot 'Nioh_Savefile_decrypt.exe'
        Link-Executable $helper @($aes,$crypto,$editor,$main)
        Get-FileHash -LiteralPath $helper -Algorithm SHA256 |
            Format-List | Out-String | Tee-Object -FilePath $compileLog -Append | Out-Host
        Write-Output "LEGACY_CRYPTO_HELPER=$helper"
    }
    $env:ASAN_OPTIONS = 'halt_on_error=1'
    & $testExe $TestCase 2>&1 | Tee-Object -FilePath (Join-Path $logs ($LogPrefix + '-' + $TestCase + '.log')) | Out-Host
    $testExit = $LASTEXITCODE
    Write-Output "LEGACY_CRYPTO_TEST_EXE=$testExe"
    Write-Output "LEGACY_CRYPTO_TEST_EXIT=$testExit"
    if ($testExit -ne 0) { throw "Legacy crypto bounds regression failed (exit $testExit)." }
} finally {
    $env:TEMP = $savedTemp
    $env:TMP = $savedTmp
    $env:ASAN_OPTIONS = $savedAsan
}