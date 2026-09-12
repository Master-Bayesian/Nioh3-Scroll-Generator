param(
    [Parameter(Mandatory)][int]$ProcessId,
    [Parameter(Mandatory)][int]$LauncherProcessId,
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Staged,
    [Parameter(Mandatory)][string]$FileHash,
    [Parameter(Mandatory)][string]$PreviousHash,
    [Parameter(Mandatory)][string]$ManifestHash,
    [Parameter(Mandatory)][string]$Profile
)
$ErrorActionPreference = 'Stop'
function Get-FileDigest([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-','').ToLowerInvariant() }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}
function Assert-RegularFile([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Update requires a regular executable file'
    }
}
function Write-Receipt($Value) {
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'last-update-result.json'), ($Value | ConvertTo-Json))
}
trap {
    $failure = $_.Exception.Message
    if ($prepared -and [IO.File]::Exists($prepared)) {
        # This is one exact sibling file created by this invocation, never a directory.
        if ([IO.Path]::GetDirectoryName($prepared) -eq $parent -and
            [IO.Path]::GetFileName($prepared) -match ('^' + [regex]::Escape([IO.Path]::GetFileName($targetPath)) + '\.update-[a-f0-9]{32}$')) {
            [IO.File]::Delete($prepared)
        }
    }
    Write-Receipt @{ status='failed'; mode='onefile'; error=$failure; time=[DateTime]::UtcNow.ToString('o') }
    exit 1
}
$targetPath = [IO.Path]::GetFullPath($Target)
$stagePath = [IO.Path]::GetFullPath($Staged)
$parent = [IO.Path]::GetDirectoryName($targetPath)
if (-not $parent -or $targetPath -eq $stagePath -or [IO.Path]::GetExtension($targetPath) -ne '.exe') { throw 'Invalid one-file update target' }
foreach ($digest in @($FileHash, $PreviousHash, $ManifestHash)) {
    if ($digest -notmatch '^[a-fA-F0-9]{64}$') { throw 'Invalid update digest' }
}
Assert-RegularFile $targetPath
Assert-RegularFile $stagePath
if ((Get-FileDigest $targetPath) -ne $PreviousHash -or (Get-FileDigest $stagePath) -ne $FileHash) { throw 'Executable changed before update' }
$prepared = Join-Path $parent ([IO.Path]::GetFileName($targetPath) + '.update-' + [guid]::NewGuid().ToString('N'))
$previous = Join-Path $parent ([IO.Path]::GetFileName($targetPath) + '.previous-' + [guid]::NewGuid().ToString('N'))
[IO.File]::Copy($stagePath, $prepared, $false)
if ((Get-FileDigest $prepared) -ne $FileHash) { throw 'Replacement verification failed' }
foreach ($waitId in @($ProcessId, $LauncherProcessId) | Select-Object -Unique) {
    if ($waitId -le 0 -or $waitId -eq $PID) { throw 'Invalid update process identifier' }
    Wait-Process -Id $waitId -Timeout 90 -ErrorAction SilentlyContinue
    if (Get-Process -Id $waitId -ErrorAction SilentlyContinue) { throw 'Application did not exit safely' }
}
Assert-RegularFile $targetPath
if ((Get-FileDigest $targetPath) -ne $PreviousHash) { throw 'Original executable changed while waiting' }
Assert-RegularFile $prepared
if ((Get-FileDigest $prepared) -ne $FileHash) { throw 'Replacement changed while waiting' }
$moved = $false
try {
    [IO.File]::Move($targetPath, $previous)
    $moved = $true
    [IO.File]::Move($prepared, $targetPath)
    Write-Receipt @{ status='awaiting-startup'; mode='onefile'; target=$targetPath; previous=$previous;
        fileHash=$FileHash.ToLowerInvariant(); previousHash=$PreviousHash.ToLowerInvariant();
        manifestHash=$ManifestHash.ToLowerInvariant(); time=[DateTime]::UtcNow.ToString('o') }
    # The replacement launcher supplies fresh context to its child.
    foreach ($name in @('NIOH3_ONEFILE_EXE','NIOH3_ONEFILE_PID','NIOH3_ONEFILE_PAYLOAD_SHA256')) {
        [Environment]::SetEnvironmentVariable($name, $null, 'Process')
    }
    Start-Process -FilePath $targetPath -WorkingDirectory $parent -ArgumentList @('--user-data-dir', ('"' + [IO.Path]::GetFullPath($Profile) + '"')) -WindowStyle Hidden
} catch {
    if ($moved) {
        if ([IO.File]::Exists($targetPath)) { [IO.File]::Delete($targetPath) }
        [IO.File]::Move($previous, $targetPath)
    }
    throw
}
# The verified replacement acknowledges startup before deleting the rollback copy.
