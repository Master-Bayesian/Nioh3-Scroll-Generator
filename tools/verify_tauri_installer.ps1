param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$Python
)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)

$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$pythonPath = (Resolve-Path -LiteralPath $Python).Path
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$smokeRoot = Join-Path $temporaryRoot ('nioh3-installer-smoke-' + [guid]::NewGuid().ToString('N'))
$installRoot = Join-Path $smokeRoot 'app'
$resolvedSmokeRoot = [IO.Path]::GetFullPath($smokeRoot).TrimEnd('\')
if (-not $resolvedSmokeRoot.StartsWith($temporaryRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Installer smoke directory escapes the system temporary directory'
}
New-Item -ItemType Directory -Path $smokeRoot | Out-Null

try {
    $install = Start-Process -FilePath $installerPath -ArgumentList @('/S', "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($install.ExitCode -ne 0) { throw "Installer exited with code $($install.ExitCode)" }

    $application = Join-Path $installRoot 'Nioh3Studio.exe'
    $uninstaller = Join-Path $installRoot 'uninstall.exe'
    $manifestPath = Join-Path $installRoot 'build-manifest.json'
    foreach ($required in @($application, $uninstaller, $manifestPath)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "Installer omitted required file: $required"
        }
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.schema -ne 'nioh3-tauri-manifest/v1' -or $manifest.git.dirty -ne $false) {
        throw 'Installed package manifest is not a clean Tauri release'
    }

    $env:NIOH3_PYTHON = $pythonPath
    $env:NIOH3_TAURI_EXE = $application
    & node.exe apps/tauri/verify.mjs
    if ($LASTEXITCODE -ne 0) { throw "Installed application verification failed with code $LASTEXITCODE" }

    $uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru -WindowStyle Hidden
    if ($uninstall.ExitCode -ne 0) { throw "Uninstaller exited with code $($uninstall.ExitCode)" }
    if (Test-Path -LiteralPath $application) { throw 'Uninstaller left the application executable behind' }
    Write-Output "TAURI_INSTALL_START_UNINSTALL_OK: $($manifest.version)"
}
finally {
    if (Test-Path -LiteralPath $smokeRoot) {
        $item = Get-Item -LiteralPath $smokeRoot -Force
        $checked = [IO.Path]::GetFullPath($item.FullName).TrimEnd('\')
        if ($checked -ne $resolvedSmokeRoot -or
            -not $checked.StartsWith($temporaryRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
            ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'Refusing to clean an unexpected installer smoke directory'
        }
        Remove-Item -LiteralPath $checked -Recurse -Force
    }
}
