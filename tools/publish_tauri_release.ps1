<#
.SYNOPSIS
  Deterministically promote the already-built, already-verified Nioh 3 Studio
  release bytes. This helper never rebuilds, signs, or renames a release asset.

.DESCRIPTION
  Default (read-only) mode:
    1. Confirm origin is the repository being promoted.
    2. Validate that -RunId is a successful release.yml run of exactly -ExpectedSha.
    3. Download only the existing `nioh3-tauri-release` artifact from that run.
    4. Stage the six assets and verify them with tools/verify_release_artifacts.py,
       run through tools/run_python_tests.ps1.
    5. Determine the branch, tag and release state, then write <Output>/plan.json.
       No remote mutation is performed.

  -Publish (explicit) repeats the read-only gate, then either verifies an existing
  matching public release (no mutation) or performs the command-scoped annotated
  tag, the atomic ref push, and the draft -> public release creation, followed by
  a public re-download verification. It never overwrites an existing tag, asset,
  or update feed, and any draft, prerelease or partial asset set stops with a
  diagnostic instead of being retried or replaced.

.EXAMPLE
  ./tools/publish_tauri_release.ps1 -RunId 35625590622 `
      -ExpectedSha 3798693c48cef2238480da66dc0cc0d2a098c78b -Version 0.8.0 `
      -Output deliverables/release-promotion
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RunId,
    [Parameter(Mandatory)][string]$ExpectedSha,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Output,
    [string]$Python = $env:NIOH3_PYTHON,
    [string]$Repository = 'Master-Bayesian/Nioh3-Scroll-Generator',
    # Command-scoped identity for the annotated tag. GitHub runners have none
    # configured globally, and this helper must never write a global git config.
    [string]$TagIdentityName = 'Nioh3 release promotion',
    [string]$TagIdentityEmail = 'nioh3-release-promotion@users.noreply.github.com',
    [switch]$Publish
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$verifierRelative = 'tools/verify_release_artifacts.py'
$verifier = Join-Path $projectRoot $verifierRelative
$pythonRunner = Join-Path $PSScriptRoot 'run_python_tests.ps1'
$artifactName = 'nioh3-tauri-release'

function Fail([string]$message) {
    throw "PROMOTION_BLOCKED: $message"
}

function Get-ReleaseAssetNames([string]$version) {
    @(
        "Nioh3Studio-$version-win-x64.exe",
        "Nioh3Studio-$version-win-x64.exe.sha256",
        "Nioh3Studio-$version-win-x64.sha256",
        "Nioh3Studio-$version-win-x64.zip",
        'tauri-update.json',
        'test-inventory.json'
    )
}

function Get-SortedNames([string[]]$names) { return @($names | Sort-Object) }

# The verifier requires a fresh (absent or empty) directory for a public
# re-download, so a repeated promotion run never reuses an earlier one.
function New-FreshPublicDirectory([string]$parent) {
    $stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss')
    $candidate = Join-Path $parent "public-redownload-$stamp"
    if (Test-Path -LiteralPath $candidate) {
        $candidate = Join-Path $parent ("public-redownload-$stamp-" + [guid]::NewGuid().ToString('N').Substring(0, 6))
    }
    return $candidate
}

function Invoke-Gh([string[]]$arguments) {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & gh @arguments 2>&1
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previous
    }
    return [pscustomobject]@{ ExitCode = $code; Output = ($output | Out-String).Trim() }
}

function Invoke-Git([string[]]$arguments) {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & git @arguments 2>&1
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previous
    }
    return [pscustomobject]@{ ExitCode = $code; Output = ($output | Out-String).Trim() }
}

# `gh api --include` prints the HTTP status line even on failure, so a missing
# release (404) can be told apart from a transport, auth or server failure.
function Invoke-GhApi([string]$endpoint) {
    $result = Invoke-Gh @('api', '--include', $endpoint)
    $status = $null
    if ($result.Output -match '(?m)^HTTP/\S+\s+(\d{3})') { $status = [int]$Matches[1] }
    return [pscustomobject]@{ ExitCode = $result.ExitCode; Status = $status; Text = $result.Output }
}

function Get-ApiBody([string]$text) {
    $lines = $text -split "`r?`n"
    for ($index = 1; $index -lt $lines.Count; $index++) {
        if ([string]::IsNullOrWhiteSpace($lines[$index])) {
            return (($lines[($index + 1)..($lines.Count - 1)]) -join "`n")
        }
    }
    return ''
}

function ConvertFrom-RemoteUrl([string]$url) {
    $value = $url.Trim()
    $value = $value -replace '^git@([^:]+):', 'https://$1/'
    $value = $value -replace '^ssh://git@', 'https://'
    $value = $value -replace '\.git$', ''
    $value = $value.TrimEnd('/')
    if ($value -match '^https?://[^/]+/(?<slug>.+)$') { return $Matches['slug'] }
    return ''
}

if ($Version -notmatch '^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$') { Fail "invalid -Version '$Version'" }
if ($ExpectedSha -notmatch '^[0-9a-f]{40}$') { Fail 'invalid -ExpectedSha (expected a full lowercase commit)' }
if ($RunId -notmatch '^\d+$') { Fail "invalid -RunId '$RunId'" }
if ([string]::IsNullOrWhiteSpace($Repository)) { Fail 'invalid -Repository' }
if (-not (Test-Path -LiteralPath $verifier -PathType Leaf)) { Fail "verifier is missing: $verifier" }
if (-not (Test-Path -LiteralPath $pythonRunner -PathType Leaf)) { Fail "python runner is missing: $pythonRunner" }
if ([string]::IsNullOrWhiteSpace($Python)) { Fail 'no Python: pass -Python or set NIOH3_PYTHON' }
if (-not (Test-Path -LiteralPath $Python -PathType Leaf)) { Fail "python does not exist: $Python" }

Write-Host "Confirming this checkout promotes $Repository"
$origin = Invoke-Git @('config', '--get', 'remote.origin.url')
if ($origin.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($origin.Output)) {
    Fail "this checkout has no 'origin' remote, so the promotion target cannot be confirmed"
}
$originSlug = ConvertFrom-RemoteUrl $origin.Output
if ($originSlug -ne $Repository) {
    Fail "origin '$($origin.Output)' resolves to '$originSlug', not -Repository '$Repository'"
}

$outputRoot = [System.IO.Path]::GetFullPath($Output)
$assetsDir = Join-Path $outputRoot 'assets'
$rawDir = Join-Path $outputRoot 'artifact'
$reportPath = Join-Path $outputRoot 'verify-report.json'
$planPath = Join-Path $outputRoot 'plan.json'
New-Item -ItemType Directory -Force -Path $outputRoot, $assetsDir, $rawDir | Out-Null

Write-Host "Validating workflow run $RunId in $Repository"
$runApi = Invoke-GhApi "repos/$Repository/actions/runs/$RunId"
if ($runApi.Status -eq 404) { Fail "workflow run $RunId was not found in $Repository" }
if ($runApi.ExitCode -ne 0 -or $runApi.Status -ne 200) {
    Fail "could not read workflow run $RunId (HTTP $($runApi.Status)): $($runApi.Text)"
}
$run = Get-ApiBody $runApi.Text | ConvertFrom-Json
if ($run.head_sha -ne $ExpectedSha) { Fail "run $RunId belongs to $($run.head_sha), not $ExpectedSha" }
if ($run.conclusion -ne 'success') { Fail "run $RunId conclusion is '$($run.conclusion)', not success" }
if ([string]::IsNullOrWhiteSpace($run.path) -or ($run.path -notmatch 'release\.yml$')) {
    Fail "run $RunId is '$($run.path)', not the release.yml preparation workflow"
}

Write-Host "Downloading the existing '$artifactName' artifact from run $RunId"
Get-ChildItem -LiteralPath $rawDir -File -Recurse -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
$download = Invoke-Gh @('run', 'download', $RunId, '--repo', $Repository, '--name', $artifactName, '--dir', $rawDir)
if ($download.ExitCode -ne 0) { Fail "artifact '$artifactName' could not be downloaded: $($download.Output)" }

$expectedNames = Get-ReleaseAssetNames $Version
$found = Get-ChildItem -LiteralPath $rawDir -File -Recurse
foreach ($name in $expectedNames) {
    $matches = @($found | Where-Object { $_.Name -eq $name })
    if ($matches.Count -ne 1) { Fail "expected exactly one '$name' in the artifact, found $($matches.Count)" }
    Copy-Item -LiteralPath $matches[0].FullName -Destination (Join-Path $assetsDir $name) -Force
}

Write-Host 'Verifying the downloaded bytes through the project Python runner'
$verifyBaseArguments = @(
    '--directory', $assetsDir,
    '--version', $Version,
    '--expected-sha', $ExpectedSha,
    '--repository', $Repository
)
$publicReportPath = Join-Path $outputRoot 'public-verify-report.json'
& $pythonRunner -Python $Python -ScriptPath $verifierRelative -ScriptArgument (@($verifyBaseArguments) + @('--report', $reportPath)) | Out-Null
if ($LASTEXITCODE -ne 0) { Fail "artifact verification failed; see $reportPath" }
$report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
if (-not $report.passed) { Fail "artifact verification reported failures; see $reportPath" }
$checks = $report.checks.Count
$assetTable = @{}
foreach ($property in $report.assets.PSObject.Properties) { $assetTable[$property.Name] = $property.Value }
Write-Host "Verified $checks checks over the six assets"

Write-Host 'Refreshing origin/main and the version tag'
$fetch = Invoke-Git @('fetch', 'origin', 'main')
if ($fetch.ExitCode -ne 0) { Fail "git fetch origin main failed: $($fetch.Output)" }

# The candidate is bound by SHA only. Fetch its commit when it is not already
# present so ancestry and tag checks work for a candidate that is newer than the
# checkout. This never moves HEAD and never touches the worktree.
if ((Invoke-Git @('cat-file', '-e', "$ExpectedSha^{commit}")).ExitCode -ne 0) {
    Write-Host "Fetching candidate $ExpectedSha into a temporary ref (HEAD and the worktree stay untouched)"
    $tempRef = "refs/nioh3-promotion/$ExpectedSha"
    $fetchCandidate = Invoke-Git @('fetch', '--no-tags', 'origin', "${ExpectedSha}:$tempRef")
    if ($fetchCandidate.ExitCode -ne 0) { $fetchCandidate = Invoke-Git @('fetch', '--no-tags', 'origin', $ExpectedSha) }
    if ($fetchCandidate.ExitCode -ne 0 -or (Invoke-Git @('cat-file', '-e', "$ExpectedSha^{commit}")).ExitCode -ne 0) {
        Fail "candidate $ExpectedSha is neither local nor fetchable from origin: $($fetchCandidate.Output)"
    }
}

$mainAncestor = (Invoke-Git @('merge-base', '--is-ancestor', 'origin/main', $ExpectedSha)).ExitCode -eq 0
$candidateAncestor = (Invoke-Git @('merge-base', '--is-ancestor', $ExpectedSha, 'origin/main')).ExitCode -eq 0

$remoteTag = Invoke-Git @('ls-remote', '--tags', 'origin', "refs/tags/v$Version", "refs/tags/v$Version^{}")
if ($remoteTag.ExitCode -ne 0) {
    Fail "git ls-remote failed, so the tag state of v$Version is unknown (not treated as absent): $($remoteTag.Output)"
}
$tagCommit = $null
if ($remoteTag.Output) {
    $peeled = @($remoteTag.Output -split "`n" | Where-Object { $_ -match "refs/tags/v$([regex]::Escape($Version))\^\{\}" })
    if ($peeled) {
        $tagCommit = ($peeled[0] -split '\s+')[0]
    } else {
        $fetchTag = Invoke-Git @('fetch', 'origin', "refs/tags/v$Version`:refs/tags/v$Version")
        if ($fetchTag.ExitCode -ne 0) { Fail "tag v$Version exists on the remote but could not be fetched: $($fetchTag.Output)" }
        $resolved = Invoke-Git @('rev-parse', "v$Version^{commit}")
        if ($resolved.ExitCode -ne 0) { Fail "tag v$Version could not be resolved to a commit: $($resolved.Output)" }
        $tagCommit = $resolved.Output
    }
}
$tagState = if (-not $tagCommit) { 'absent' } elseif ($tagCommit -eq $ExpectedSha) { 'same' } else { 'different' }
if ($tagState -eq 'different') { Fail "tag v$Version already exists at $tagCommit and will never be moved" }

# A new tag requires main to fast-forward to the candidate. An already-published
# tag accepts a main that has moved on with later documentation commits, as long
# as the candidate is part of main's history.
if ($tagState -ne 'same') {
    if (-not $mainAncestor) { Fail "origin/main is not an ancestor of $ExpectedSha (non fast-forward; resolve the product/docs split before publishing)" }
} elseif (-not ($mainAncestor -or $candidateAncestor)) {
    Fail "neither origin/main nor $ExpectedSha is an ancestor of the other; the candidate is unrelated to main"
}
$mainPushRequired = $mainAncestor -and -not $candidateAncestor

$releaseApi = Invoke-GhApi "repos/$Repository/releases/tags/v$Version"
$releaseState = 'absent'
$releaseAssetNames = @()
if ($releaseApi.Status -eq 404) {
    $releaseState = 'absent'
} elseif ($releaseApi.ExitCode -ne 0 -or $releaseApi.Status -ne 200) {
    Fail "could not read the release state of v$Version (HTTP $($releaseApi.Status)); this is not treated as absent: $($releaseApi.Text)"
} else {
    $release = Get-ApiBody $releaseApi.Text | ConvertFrom-Json
    $releaseAssetNames = Get-SortedNames @($release.assets | ForEach-Object { $_.name })
    if ($release.draft) {
        $releaseState = 'draft'
    } elseif ($release.prerelease) {
        $releaseState = 'prerelease'
    } else {
        $digests = @{}
        foreach ($entry in $release.assets) { if ($entry.digest) { $digests[$entry.name] = $entry.digest } }
        $matchesAll = ($releaseAssetNames -join ',') -eq ((Get-SortedNames $expectedNames) -join ',')
        foreach ($name in $expectedNames) {
            if (-not $digests.ContainsKey($name) -or $digests[$name] -ne "sha256:$($assetTable[$name].sha256)") { $matchesAll = $false }
        }
        $releaseState = if ($matchesAll) { 'matching' } else { 'mismatched' }
    }
}

$tagArguments = @('-c', "user.name=$TagIdentityName", '-c', "user.email=$TagIdentityEmail", 'tag', '-a', "v$Version", $ExpectedSha, '-m', "Release v$Version")
$refUpdates = @()
if ($mainPushRequired) { $refUpdates += "${ExpectedSha}:refs/heads/main" }
if ($tagState -eq 'absent') { $refUpdates += "refs/tags/v$Version" }
$pushArguments = if ($refUpdates.Count) { @('push', '--atomic', 'origin') + $refUpdates } else { @() }

$plannedMutations = @()
if ($releaseState -eq 'absent') {
    if ($tagState -eq 'absent') { $plannedMutations += ('git ' + ($tagArguments -join ' ')) }
    if ($pushArguments.Count) { $plannedMutations += ('git ' + ($pushArguments -join ' ')) }
    $plannedMutations += "gh release create v$Version --repo $Repository --draft --verify-tag (upload the six verified assets)"
    $plannedMutations += "gh release edit v$Version --repo $Repository --draft=false --latest"
}

$plan = [ordered]@{
    generated_at_utc  = (Get-Date).ToUniversalTime().ToString('o')
    mode              = if ($Publish) { 'publish' } else { 'plan' }
    repository        = $Repository
    origin            = $origin.Output
    version           = $Version
    candidate_sha     = $ExpectedSha
    run_id            = $RunId
    run_url           = $run.html_url
    run_head_branch   = $run.head_branch
    artifact          = $artifactName
    verifier_checks   = $checks
    verify_report     = $reportPath
    assets            = @($expectedNames | ForEach-Object {
            [ordered]@{ name = $_; bytes = $assetTable[$_].bytes; sha256 = $assetTable[$_].sha256 }
        })
    main_ancestor_of_candidate    = $mainAncestor
    candidate_ancestor_of_main    = $candidateAncestor
    main_push_required            = $mainPushRequired
    tag_state         = $tagState
    release_state     = $releaseState
    release_assets    = $releaseAssetNames
    planned_mutations = $plannedMutations
}
$plan | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $planPath -Encoding utf8
Write-Host "Plan written to $planPath (tag=$tagState release=$releaseState)"

if ($releaseState -eq 'draft') {
    Fail "a DRAFT release already exists for v$Version; it is partial state, not a matching public release. Inspect and resolve it manually instead of overwriting it."
}
if ($releaseState -eq 'prerelease') {
    Fail "release v$Version exists as a PRERELEASE; it is not the matching public release. Inspect it manually."
}
if ($releaseState -eq 'mismatched') {
    Fail "release v$Version already exists with assets [$($releaseAssetNames -join ', ')] that do not match the verified bytes; inspect and resolve it manually instead of overwriting assets."
}

if (-not $Publish) {
    Write-Host 'Read-only plan complete; no remote mutation was performed.'
    exit 0
}

$resultPath = Join-Path $outputRoot 'publish-result.json'
function Write-PublishResult([string]$state, [string]$detail) {
    [ordered]@{
        generated_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        version          = $Version
        candidate_sha    = $ExpectedSha
        state            = $state
        detail           = $detail
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $resultPath -Encoding utf8
}

$notes = $null
try { $notes = (Get-Content -LiteralPath (Join-Path $assetsDir 'tauri-update.json') -Raw | ConvertFrom-Json).notes } catch { }
$notesPath = Join-Path $outputRoot 'release-notes.md'
if ([string]::IsNullOrWhiteSpace($notes)) { $notes = "Nioh 3 Studio v$Version" }
Set-Content -LiteralPath $notesPath -Value $notes -Encoding utf8

if ($releaseState -eq 'matching') {
    Write-Host 'An existing public release already matches the verified bytes; verifying only.'
    $publicDir = New-FreshPublicDirectory $outputRoot
    & $pythonRunner -Python $Python -ScriptPath $verifierRelative -ScriptArgument (@($verifyBaseArguments) + @('--report', $publicReportPath, '--public', '--public-download-dir', $publicDir)) | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-PublishResult 'existing-matching-unverified' 'public re-download verification failed'; Fail 'existing matching release failed public verification; see public-verify-report.json' }
    Write-PublishResult 'existing-matching-release' 'verified only; nothing was mutated'
    exit 0
}

try {
    if ($tagState -eq 'absent') {
        $tag = Invoke-Git $tagArguments
        if ($tag.ExitCode -ne 0) { Fail "could not create the annotated tag: $($tag.Output)" }
    }
    if ($pushArguments.Count) {
        $push = Invoke-Git $pushArguments
        if ($push.ExitCode -ne 0) { Fail "atomic push failed ($($refUpdates -join ' ')): $($push.Output)" }
        Write-Host "Pushed $($refUpdates.Count) ref update(s) atomically: $($refUpdates -join ' ')"
    } else {
        Write-Host "Tag v$Version already points at the candidate and main already contains it; no ref push needed."
    }

    $assetPaths = $expectedNames | ForEach-Object { Join-Path $assetsDir $_ }
    $create = Invoke-Gh (@('release', 'create', "v$Version", '--repo', $Repository, '--draft', '--verify-tag',
            '--title', "Nioh 3 Studio v$Version", '--notes-file', $notesPath) + $assetPaths)
    if ($create.ExitCode -ne 0) { Fail "draft release creation or asset upload failed: $($create.Output)" }

    $promote = Invoke-Gh @('release', 'edit', "v$Version", '--repo', $Repository, '--draft=false', '--latest')
    if ($promote.ExitCode -ne 0) { Fail "could not make the release public/latest: $($promote.Output)" }
} catch {
    Write-PublishResult 'publish-failed' $_.Exception.Message
    throw
}

$publicDir = New-FreshPublicDirectory $outputRoot
& $pythonRunner -Python $Python -ScriptPath $verifierRelative -ScriptArgument (@($verifyBaseArguments) + @('--report', $publicReportPath, '--public', '--public-download-dir', $publicDir)) | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-PublishResult 'published-unverified' 'public re-download verification failed after publication'
    Fail 'publication completed but the public re-download verification failed; see public-verify-report.json'
}
Write-PublishResult 'published-verified' 'main, tag, and public release created and re-verified from the public downloads'
Write-Host 'Publication complete and re-verified from the public downloads.'
