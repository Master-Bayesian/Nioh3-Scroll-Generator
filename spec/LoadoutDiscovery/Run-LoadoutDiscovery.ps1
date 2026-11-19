#Requires -Version 7
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Evidence,
    [string]$BuildRoot = 'F:/Nioh3_r16_build',
    [string]$Java = 'F:/Nioh3_r16_build/tools/temurin-21/jdk-21.0.12.1+1/bin/java.exe'
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $Evidence) { throw 'Choose a new discovery evidence directory.' }
$quality = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$checker = Join-Path $quality 'tools/check_models.ps1'
$jar = Join-Path $BuildRoot 'tools/tla/tla2tools.jar'
$expectedJarHash = 'e1f11b75c4b5bc4478ba0ac5cb6ae28e40a01b0016bc33e0aa97da22dfdbcd7f'
foreach ($path in @($checker, $Java, $jar)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Required pinned model tool is absent: $path" }
}
if ((Get-FileHash -LiteralPath $jar -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedJarHash) {
    throw 'Pinned TLC identity differs.'
}
New-Item -ItemType Directory -Path $Evidence | Out-Null
$taskTemp = Join-Path $BuildRoot 'tmp'
New-Item -ItemType Directory -Path $taskTemp -Force | Out-Null
$saved = @{}
foreach ($name in @('NIOH3_BUILD_ROOT','TEMP','TMP','JAVA_TOOL_OPTIONS')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
try {
    [Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
    $env:NIOH3_BUILD_ROOT = $BuildRoot
    $env:TEMP = $taskTemp
    $env:TMP = $taskTemp
    $env:JAVA_TOOL_OPTIONS = '-XX:ActiveProcessorCount=1 -Xmx256m'
    & $checker -Model LoadoutDiscovery -Java $Java -Evidence $Evidence *> (Join-Path $Evidence 'MODEL_CHECK.log')
    if ($LASTEXITCODE -ne 0) { throw 'Discovery model/fault gate failed; inspect MODEL_CHECK.log.' }
    $controls = @(Get-Content -LiteralPath (Join-Path $Evidence 'model-controls.json') -Raw | ConvertFrom-Json)
    if ($controls.Count -ne 3 -or ($controls | Where-Object { -not $_.pass }).Count) { throw 'Expected exactly one correct run and two named counterexamples.' }
    $counterexamples = foreach ($control in $controls | Where-Object expected) {
        $text = Get-Content -LiteralPath $control.log -Raw
        $blocks = [regex]::Matches($text, '(?ms)^State (?<step>\d+): (?<action>[^\r\n]+)\r?\n(?<body>.*?)(?=^State \d+:|^\d+ states generated)')
        if ($blocks.Count -lt 2) { throw 'A named fault has no concrete counterexample states.' }
        $states = foreach ($block in $blocks) {
            $fields = @{}
            foreach ($row in [regex]::Matches($block.Groups['body'].Value, '(?m)^/\\ (?<name>\w+) = (?<value>[^\r\n]+)')) {
                $fields[$row.Groups['name'].Value] = $row.Groups['value'].Value.Trim()
            }
            [ordered]@{
                step=[int]$block.Groups['step'].Value;action=$block.Groups['action'].Value
                state=$fields['state'].Trim('"');requestId=[int]$fields['requestId']
                acceptedResponseId=[int]$fields['acceptedResponseId']
                dataPresent=[bool]::Parse($fields['dataPresent']);dataRequestId=[int]$fields['dataRequestId']
                errorPresent=[bool]::Parse($fields['errorPresent']);errorRequestId=[int]$fields['errorRequestId']
                outcome=$fields['outcome'].Trim('"');issued=$fields['issued']
            }
        }
        [ordered]@{case=$control.case;expected=$control.expected;exitCode=$control.exitCode;states=@($states);log=$control.log}
    }
    $counterexamples | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $Evidence 'DISCOVERY_COUNTEREXAMPLES.json') -Encoding utf8
    $sourceFiles = @('LoadoutDiscovery.tla','LoadoutDiscovery.cfg','LoadoutDiscovery.negative.json','Run-LoadoutDiscovery.ps1','README.md')
    $files = foreach ($name in $sourceFiles) {
        $path = Join-Path $PSScriptRoot $name
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            Copy-Item -LiteralPath $path -Destination (Join-Path $Evidence $name)
            [pscustomobject]@{path=$path;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
        }
    }
    $implementation = Join-Path $quality 'apps/workshop/loadout-discovery-state.ts'
    $implementationExists = Test-Path -LiteralPath $implementation -PathType Leaf
    $receipt = [ordered]@{
        schema='nioh3-loadout-discovery-model/v1';passed=$true;maxRequests=2
        toolchain=@{java=$Java;tlc=$jar;tlc_sha256=$expectedJarHash;workers=1;activeProcessors=1;heapMiB=256;priority='Idle';temporaryRoot=$taskTemp}
        sourceFiles=$files;controls=$controls;counterexamples=(Join-Path $Evidence 'DISCOVERY_COUNTEREXAMPLES.json')
        implementation=@{path=$implementation;present=$implementationExists;sha256=$(if($implementationExists){(Get-FileHash -LiteralPath $implementation -Algorithm SHA256).Hash.ToLowerInvariant()}else{$null});equivalenceClaimed=$false}
        scope='Finite reducer and visibility safety. No browser/native UI/backend availability or async fairness proof.'
    }
    $receipt | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $Evidence 'DISCOVERY_MODEL_ACCEPTANCE.json') -Encoding utf8
    $hashes = foreach ($file in Get-ChildItem -LiteralPath $Evidence -File) {
        [pscustomobject]@{path=$file.Name;sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    }
    $hashes | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $Evidence 'SHA256SUMS.json') -Encoding utf8
    Write-Output ('Discovery model and both exact invariant faults passed: ' + $Evidence)
} finally {
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}