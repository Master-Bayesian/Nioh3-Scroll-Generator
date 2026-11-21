#Requires -Version 7
$ErrorActionPreference='Stop'
$buildRoot=if($env:NIOH3_BUILD_ROOT){[IO.Path]::GetFullPath($env:NIOH3_BUILD_ROOT)}else{'D:\Nioh3_v080_deliverables'}
$toolRoot=Join-Path $buildRoot 'tools/actionlint/1.7.12'
New-Item -ItemType Directory -Force -Path $toolRoot | Out-Null
$archive=Join-Path $toolRoot 'actionlint.zip'
if(-not (Test-Path -LiteralPath $archive)){
    Invoke-WebRequest 'https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_windows_amd64.zip' -OutFile $archive
}
if((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne '6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9'){
    throw 'Pinned actionlint archive digest mismatch'
}
$zip=[IO.Compression.ZipFile]::OpenRead($archive)
try {
    $entry=$zip.GetEntry('actionlint.exe')
    if($null -eq $entry){throw 'Pinned archive has no actionlint.exe'}
    $stream=$entry.Open()
    try {$memory=[IO.MemoryStream]::new();$stream.CopyTo($memory);$bytes=$memory.ToArray();$memory.Dispose()} finally {$stream.Dispose()}
} finally {$zip.Dispose()}
$executable=Join-Path $toolRoot 'actionlint.exe'
# Extract exactly the pinned binary rather than arbitrary archive paths.
if(-not (Test-Path -LiteralPath $executable) -or (Get-FileHash -LiteralPath $executable).Hash -ne [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes))){[IO.File]::WriteAllBytes($executable,$bytes)}
& $executable -color=false .github/workflows/tests.yml .github/workflows/release.yml .github/workflows/publish-release.yml
if($LASTEXITCODE -ne 0){throw 'Workflow syntax/context lint failed'}
foreach($path in @('tools/ci/run-native-packaging.ps1','tools/ci/check-workflows.ps1')){
    $parseTokens=$null;$parseErrors=$null
    [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PWD $path),[ref]$parseTokens,[ref]$parseErrors)
    if($parseErrors.Count){throw ($parseErrors | Out-String)}
}
Write-Output 'Workflow/context lint and PowerShell syntax checks passed.'
