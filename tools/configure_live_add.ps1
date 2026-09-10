param(
    [string]$PortableRoot,
    [string]$Output
)
$ErrorActionPreference = 'Stop'
if (-not $PortableRoot -and (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'Nioh3ScrollEditorV2.exe'))) {
    $PortableRoot = $PSScriptRoot
}
if (-not $Output) {
    $Output = Join-Path $env:LOCALAPPDATA ('Nioh3ScrollEditor/live-add-connections/' + [guid]::NewGuid().ToString('N'))
}
$connectionRoot = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $connectionRoot) { throw 'Choose a new connection directory.' }
if ($PortableRoot) {
    $applicationRoot = (Resolve-Path -LiteralPath $PortableRoot).Path
    $scriptRoot = Join-Path $applicationRoot 'resources/live-add'
    $executable = Join-Path $applicationRoot 'Nioh3ScrollEditorV2.exe'
    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Portable executable is missing.' }
} else {
    $applicationRoot = Split-Path -Parent $PSScriptRoot
    $scriptRoot = Join-Path $applicationRoot 'research'
}
if (-not (Test-Path -LiteralPath (Join-Path $scriptRoot 'live_add_ce_server.lua') -PathType Leaf)) {
    throw 'Live-add executor scripts are missing.'
}
$pipeName = 'nioh3-live-add-' + [guid]::NewGuid().ToString('N')
$tokenBytes = New-Object byte[] 32
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
try { $rng.GetBytes($tokenBytes) } finally { $rng.Dispose() }
$connectionToken = -join ($tokenBytes | ForEach-Object { $_.ToString('x2') })
New-Item -ItemType Directory -Path $connectionRoot | Out-Null
$luaRoot = $scriptRoot.Replace('\', '/') + '/'
# JSON string quoting is used for Lua literals only, never as shell escaping.
$luaPathLiteral = ConvertTo-Json ($luaRoot + 'live_add_ce_server.lua') -Compress
$luaRootLiteral = ConvertTo-Json $luaRoot -Compress
$lua = "return dofile($luaPathLiteral).start('$pipeName','$connectionToken',$luaRootLiteral)"
[IO.File]::WriteAllText((Join-Path $connectionRoot 'connect-ce.lua'), $lua, [Text.UTF8Encoding]::new($false))
$quotedRoot = $applicationRoot.Replace("'", "''")
$launch = @"
`$ErrorActionPreference = 'Stop'
`$env:NIOH3_LIVE_ADD_EXECUTOR = 'ce'
`$env:NIOH3_REVIEW_UI = '1'
`$env:NIOH3_LIVE_ADD_PIPE = '$pipeName'
`$env:NIOH3_LIVE_ADD_TOKEN = '$connectionToken'
Set-Location -LiteralPath '$quotedRoot'
"@
if ($PortableRoot) {
    $launch += "`n& '.\Nioh3ScrollEditorV2.exe'`n"
} else {
    $launch += "`n& npm.cmd run desktop`n"
}
[IO.File]::WriteAllText((Join-Path $connectionRoot 'launch-v2.ps1'), $launch, [Text.UTF8Encoding]::new($false))
$guide = @'
Optional CE-backed live addition (PC v2.01 only)

1. Start the game and load a save at a quiet shrine; normally save first.
2. Attach CE's debugger to Nioh3. Close other live-add executor sessions.
3. Run connect-ce.lua once in CE's Lua script window. It creates only a local
   typed pipe; it does not add a scroll or alter the game by itself.
4. Close any existing V2 instance, then run launch-v2.ps1. This explicitly selects
   the optional CE transport instead of the default independent native executor.
   Use the connected cart controls to review and submit the selected items.
5. Keep CE open until the operation has a verified receipt. A disconnect after
   dispatch must be recovered, never retried. Normally save in game afterwards.

The connection token stays in these local files. Do not publish this directory.
Restarting CE requires running connect-ce.lua again, but must not replay any
previous insertion. Stop unsupported/conflicting code patches before preparation.
This adapter is optional; ordinary search and save operations do not require CE.
'@
[IO.File]::WriteAllText((Join-Path $connectionRoot 'README.txt'), $guide, [Text.UTF8Encoding]::new($false))
Write-Output $connectionRoot
