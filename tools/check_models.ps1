#Requires -Version 7
# Model-check every spec under spec/ with the pinned TLA+ tools.
$ErrorActionPreference = 'Stop'
$version = 'v1.8.0'
$sha256 = 'E1F11B75C4B5BC4478BA0AC5CB6AE28E40A01B0016BC33E0AA97DA22DFDBCD7F'
$cache = if ($env:NIOH3_BUILD_ROOT) { Join-Path $env:NIOH3_BUILD_ROOT 'tools/tla' } else { Join-Path $env:RUNNER_TEMP 'tla' }
New-Item -ItemType Directory -Force $cache | Out-Null
$jar = Join-Path $cache 'tla2tools.jar'
if (-not (Test-Path $jar)) {
    Invoke-WebRequest "https://github.com/tlaplus/tlaplus/releases/download/$version/tla2tools.jar" -OutFile $jar
}
if ((Get-FileHash $jar -Algorithm SHA256).Hash -ne $sha256) { throw 'tla2tools.jar does not match the pinned digest' }
$failed = 0
foreach ($spec in Get-ChildItem (Join-Path $PSScriptRoot '../spec') -Recurse -Filter *.tla) {
    $meta = Join-Path $cache "states-$($spec.BaseName)"
    Push-Location $spec.DirectoryName
    & java -cp $jar tlc2.TLC -metadir $meta -cleanup -config "$($spec.BaseName).cfg" $spec.Name
    if ($LASTEXITCODE -ne 0) { $failed = 1 }
    Pop-Location
}
exit $failed
