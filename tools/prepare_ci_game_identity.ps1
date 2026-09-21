[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Root,
    [Parameter(Mandatory)][string]$GameFileVersion,
    [switch]$ExportForActions
)
$ErrorActionPreference = 'Stop'

# Acceptance prerequisites, not a game substitute: reject malformed versions,
# existing files and accidental export on a developer host. The emitted PE is
# never executed and grants no native-write authority. Real host startup still
# discovers the Steam path and reads the Windows VERSIONINFO resource itself.
if ($GameFileVersion -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Expected a four-part game file version' }
foreach ($part in $GameFileVersion.Split('.')) {
    if ([uint64]$part -gt 65535) { throw 'Game file version component exceeds 65535' }
}
if ($ExportForActions -and ($env:GITHUB_ACTIONS -ne 'true' -or -not $env:GITHUB_ENV)) {
    throw 'Environment export is restricted to GitHub Actions'
}
$fixtureRoot = [IO.Path]::GetFullPath($Root)
$programFiles = Join-Path $fixtureRoot 'ProgramFiles'
$gameDirectory = Join-Path $programFiles 'Steam/steamapps/common/Nioh3'
$fixtureExe = Join-Path $gameDirectory 'Nioh3.exe'
$sourceFile = Join-Path $fixtureRoot 'GameIdentityFixture.cs'
if (Test-Path -LiteralPath $fixtureRoot) { throw "Use a fresh fixture directory: $fixtureRoot" }
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (-not (Test-Path -LiteralPath $compiler)) { throw 'Windows C# compiler is unavailable' }
New-Item -ItemType Directory -Force -Path $gameDirectory | Out-Null
@"
using System.Reflection;
[assembly: AssemblyVersion("$GameFileVersion")]
[assembly: AssemblyFileVersion("$GameFileVersion")]
[assembly: AssemblyDescription("Nioh3 CI version identity fixture; never execute")]
public static class GameIdentityFixture { }
"@ | Set-Content -LiteralPath $sourceFile -Encoding utf8
& $compiler /nologo /target:library /platform:x64 "/out:$fixtureExe" $sourceFile
if ($LASTEXITCODE -ne 0) { throw "Version fixture compilation failed: $LASTEXITCODE" }
$identity = [Diagnostics.FileVersionInfo]::GetVersionInfo($fixtureExe)
$observed = '{0}.{1}.{2}.{3}' -f $identity.FileMajorPart,$identity.FileMinorPart,$identity.FileBuildPart,$identity.FilePrivatePart
if ($observed -ne $GameFileVersion) { throw "Fixture VERSIONINFO mismatch: $observed" }
$evidence = [ordered]@{
    scope = 'Synthetic version-resource fixture only; never executed; no game or native-write acceptance'
    path = $fixtureExe
    fileVersion = $observed
    sha256 = (Get-FileHash -LiteralPath $fixtureExe -Algorithm SHA256).Hash.ToLowerInvariant()
}
$evidence | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $fixtureRoot 'identity.json') -Encoding utf8
if ($ExportForActions) {
    # All UI verifiers inherit this bounded Steam root. WebView2's real runtime
    # was resolved earlier, before verifiers isolate their user-data folders.
    "ProgramFiles(x86)=$programFiles" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
}
$evidence | ConvertTo-Json
