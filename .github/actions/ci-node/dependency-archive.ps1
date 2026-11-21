#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('Pack','Restore')][string]$Operation,
    [Parameter(Mandatory)][string]$ArchiveDirectory,
    [ValidateSet('success','failure','cancelled','skipped')][string]$DownloadOutcome = 'skipped'
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$archive = Join-Path $ArchiveDirectory 'node_modules.tar'
$manifestPath = Join-Path $ArchiveDirectory 'manifest.json'
$workspace = (Get-Location).Path
$installed = Join-Path $workspace 'node_modules'

function Get-DependencyIdentity {
    [ordered]@{
        source = (git rev-parse HEAD)
        invocation = "$($env:GITHUB_RUN_ID):$($env:GITHUB_RUN_ATTEMPT)"
        lockHash = (Get-FileHash -LiteralPath package-lock.json).Hash
        manifestHash = (Get-FileHash -LiteralPath package.json).Hash
        runtime = (node -p 'JSON.stringify([process.platform,process.arch,process.version,process.versions.modules])')
        npm = (npm --version)
        install = 'npm ci --no-fund'
    }
}

function Publish-RestoreOutcome([string]$Mode, [string]$Reason) {
    "mode=$Mode" >> $env:GITHUB_OUTPUT
    Write-Output "CI_DEPENDENCIES mode=$Mode reason=$Reason"
}

if ($Operation -eq 'Pack') {
    if (!(Test-Path -LiteralPath (Join-Path $installed '.bin/playwright.cmd'))) { throw 'Expected locked Windows frontend dependencies' }
    New-Item -ItemType Directory -Path $ArchiveDirectory | Out-Null
    tar.exe -cf $archive -C $workspace node_modules
    $identity = Get-DependencyIdentity
    $identity.archiveHash = (Get-FileHash -LiteralPath $archive).Hash
    $identity.archiveBytes = (Get-Item -LiteralPath $archive).Length
    $identity | ConvertTo-Json | Set-Content -LiteralPath $manifestPath -Encoding utf8
    return
}

if (Test-Path -LiteralPath $installed) { throw 'Expected a clean browser dependency destination' }
if ($DownloadOutcome -ne 'success') {
    Publish-RestoreOutcome 'npm-ci' 'artifact-unavailable'
    return
}

try {
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json -AsHashtable
    $expected = Get-DependencyIdentity
    foreach ($name in $expected.Keys) {
        if ($manifest[$name] -cne $expected[$name]) { throw "Dependency identity does not match $name" }
    }
    if ($manifest.archiveBytes -ne (Get-Item -LiteralPath $archive).Length -or
        $manifest.archiveHash -cne (Get-FileHash -LiteralPath $archive).Hash) { throw 'Dependency archive content does not match its manifest' }

    # A failed extraction leaves the checkout clean for npm ci.
    $staging = Join-Path $env:NIOH3_BUILD_ROOT "deps-$([guid]::NewGuid())"
    New-Item -ItemType Directory -Path $staging | Out-Null
    tar.exe -xf $archive -C $staging
    $stagedModules = Join-Path $staging 'node_modules'
    if (!(Test-Path -LiteralPath (Join-Path $stagedModules '.bin/playwright.cmd'))) { throw 'Expected npm-installed Windows shims' }
    [IO.Directory]::Move($stagedModules, $installed)
} catch {
    Write-Warning $_.Exception.Message
    Publish-RestoreOutcome 'npm-ci' 'artifact-rejected'
    return
}
Publish-RestoreOutcome 'artifact' 'verified'
