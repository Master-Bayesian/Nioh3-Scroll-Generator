param([Parameter(Mandatory)][int]$ProcessId,[Parameter(Mandatory)][string]$Target,[Parameter(Mandatory)][string]$Staged,[Parameter(Mandatory)][string]$ManifestHash,[Parameter(Mandatory)][string]$Profile)
$ErrorActionPreference='Stop'
function Get-PackageHash([string]$Path) {
    $stream=[IO.File]::OpenRead($Path)
    $algorithm=[Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-','') }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}
trap {
    $failure=$_.Exception.Message
    foreach($disposable in @($prepared,$failed)) {
      if($disposable -and (Test-Path -LiteralPath $disposable)) {
        # Only this invocation's newly copied sibling is disposable on failure.
        if([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($disposable)) -eq $parent -and
           [IO.Path]::GetFileName($disposable) -match ('^'+[regex]::Escape([IO.Path]::GetFileName($targetPath))+'\.(update|failed)-[a-f0-9]{32}$')) {
            $links=Get-ChildItem -LiteralPath $disposable -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }
            if(-not $links -and -not ((Get-Item -LiteralPath $disposable).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                Remove-Item -LiteralPath $disposable -Recurse -Force
            }
        }
      }
    }
    $report=@{status='failed';error=$failure;time=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'last-update-result.json'),$report)
    exit 1
}
$targetPath=[IO.Path]::GetFullPath($Target).TrimEnd('\')
$stagePath=[IO.Path]::GetFullPath($Staged).TrimEnd('\')
$parent=[IO.Path]::GetDirectoryName($targetPath)
if(-not $parent -or $targetPath -eq [IO.Path]::GetPathRoot($targetPath)){throw 'Invalid application directory'}
if($stagePath.StartsWith($targetPath+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Staged update is inside the running package'}
foreach($folder in @($targetPath,$stagePath)){
    if(-not (Test-Path -LiteralPath (Join-Path $folder 'Nioh3Studio.exe')) -or -not (Test-Path -LiteralPath (Join-Path $folder 'build-manifest.json'))){throw 'Not a complete application package'}
}
# Copy before waiting so failures leave the running installation intact. Keep rollback data.
$prepared=Join-Path $parent ([IO.Path]::GetFileName($targetPath)+'.update-'+[guid]::NewGuid().ToString('N'))
$previous=Join-Path $parent ([IO.Path]::GetFileName($targetPath)+'.previous-'+[guid]::NewGuid().ToString('N'))
foreach($candidate in @($prepared,$previous)){if([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($candidate)) -ne $parent){throw 'Replacement path escapes parent'}}
Copy-Item -LiteralPath $stagePath -Destination $prepared -Recurse
$manifestPath=Join-Path $prepared 'build-manifest.json'
if((Get-PackageHash $manifestPath) -ne $ManifestHash){throw 'Staged manifest changed'}
$manifest=Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
foreach($entry in $manifest.files){
    $file=[IO.Path]::GetFullPath([IO.Path]::Combine($prepared,$entry.path))
    if(-not $file.StartsWith($prepared+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Package path escapes replacement'}
    if((Get-Item -LiteralPath $file).Length -ne $entry.size -or (Get-PackageHash $file) -ne $entry.sha256){throw 'Staged package changed'}
}
# Tauri's NSIS installer keeps its uninstaller beside the application. Preserve
# that one installer-owned file so an in-place portable update does not remove
# the user's normal uninstall path. It is deliberately outside the portable
# manifest and is validated again before rollback cleanup.
$installedUninstaller=Join-Path $targetPath 'uninstall.exe'
$preparedUninstaller=Join-Path $prepared 'uninstall.exe'
if(Test-Path -LiteralPath $installedUninstaller){
    $uninstaller=Get-Item -LiteralPath $installedUninstaller -Force
    if(($uninstaller.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $uninstaller.PSIsContainer){throw 'Installed uninstaller is not a regular file'}
    if($uninstaller.Length -le 0 -or $uninstaller.Length -gt 64MB){throw 'Installed uninstaller is invalid'}
    if(Test-Path -LiteralPath $preparedUninstaller){throw 'Update package contains an unexpected uninstaller'}
    Copy-Item -LiteralPath $installedUninstaller -Destination $preparedUninstaller
    if((Get-Item -LiteralPath $preparedUninstaller).Length -ne $uninstaller.Length -or (Get-PackageHash $preparedUninstaller) -ne (Get-PackageHash $installedUninstaller)){throw 'Installed uninstaller copy failed'}
}
Wait-Process -Id $ProcessId -Timeout 90 -ErrorAction SilentlyContinue
if(Get-Process -Id $ProcessId -ErrorAction SilentlyContinue){throw 'Application did not exit safely'}
$moved=$false
try {
    [IO.Directory]::Move($targetPath,$previous)
    $moved=$true
    [IO.Directory]::Move($prepared,$targetPath)
    # Persist before launch: the new app may finish startup before this helper exits.
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'last-update-result.json'),(@{status='awaiting-startup';target=$targetPath;previous=$previous;manifestHash=$ManifestHash;time=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json))
    Start-Process -FilePath (Join-Path $targetPath 'Nioh3Studio.exe') -WorkingDirectory $targetPath -ArgumentList @('--user-data-dir',('"'+[IO.Path]::GetFullPath($Profile)+'"')) -WindowStyle Hidden
} catch {
    if($moved){
        if(Test-Path -LiteralPath $targetPath){
            $failed=Join-Path $parent ([IO.Path]::GetFileName($targetPath)+'.failed-'+[guid]::NewGuid().ToString('N'))
            if([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($failed)) -ne $parent){throw 'Rollback path escapes parent'}
            [IO.Directory]::Move($targetPath,$failed)
        }
        [IO.Directory]::Move($previous,$targetPath)
    }
    throw
}

# Startup acknowledgement and cleanup belong to the verified new application.
