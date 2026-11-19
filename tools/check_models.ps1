#Requires -Version 7
# Check the real models and require named counterexamples from each declared faulty variant.
param([string]$Evidence = $env:NIOH3_QUALITY_EVIDENCE, [string]$Java = 'java', [string[]]$Model = @())
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$version = 'v1.8.0'
# Official v1.8.0 asset digest independently verified through GitHub's release API
# after its 2026-11-15 asset replacement (4,510,884 bytes).
$sha256 = '7E53AA636DAC8A3CB0533F343115C0548E03FC0CE5EDC7739BECF58D3D0FB84F'
$root = if ($env:NIOH3_BUILD_ROOT) { $env:NIOH3_BUILD_ROOT } elseif ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP 'nioh3-quality' } else { 'D:/Nioh3_v080_deliverables' }
$cache = Join-Path $root 'tools/tla'
if (-not $Evidence) { $Evidence = Join-Path $root 'deliverables/quality-controls' }
New-Item -ItemType Directory -Force $cache,$Evidence | Out-Null
$jar = Join-Path $cache 'tla2tools.jar'
if (-not (Test-Path -LiteralPath $jar)) { Invoke-WebRequest "https://github.com/tlaplus/tlaplus/releases/download/$version/tla2tools.jar" -OutFile $jar }
if ((Get-FileHash -LiteralPath $jar -Algorithm SHA256).Hash -ne $sha256) { throw 'tla2tools.jar does not match the pinned digest' }
$run = Join-Path $cache ('check-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run | Out-Null
$results = [System.Collections.Generic.List[object]]::new()
function Check-Model($Spec, [string]$Name, [string]$Config, [string]$Expected) {
    $directory = Join-Path $run ($Spec.BaseName + '-' + $Name)
    New-Item -ItemType Directory -Path $directory | Out-Null
    # TLC counterexample helpers, when emitted, stay in this run's build directory.
    Copy-Item -LiteralPath $Spec.FullName -Destination (Join-Path $directory $Spec.Name)
    $configPath = Join-Path $directory ($Spec.BaseName + '.cfg')
    [IO.File]::WriteAllText($configPath, $Config)
    Push-Location -LiteralPath $directory
    try {
        $lines = @(& $Java -XX:+UseParallelGC -cp $jar tlc2.TLC -metadir (Join-Path $directory 'states') -cleanup -config $configPath $Spec.Name 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    $text = $lines -join "`n"
    $log = Join-Path $Evidence ($Spec.BaseName + '-' + $Name + '.log')
    [IO.File]::WriteAllText($log, $text + "`n")
    $pass = if ($Expected) { $code -eq 12 -and [regex]::IsMatch($text, '(?m)^Error: ' + [regex]::Escape($Expected) + '\.?\s*$') } else { $code -eq 0 -and $text.Contains('No error has been found.', [StringComparison]::Ordinal) }
    $savedConfig = Join-Path $Evidence ($Spec.BaseName + '-' + $Name + '.cfg')
    Copy-Item -LiteralPath $configPath -Destination $savedConfig
    $results.Add([pscustomobject]@{model=$Spec.BaseName;case=$Name;expected=$Expected;exitCode=$code;pass=$pass;log=$log;configuration=$savedConfig})
    "{0}/{1}: pass={2}, exit={3}{4}" -f $Spec.BaseName,$Name,$pass,$code,$(if($Expected){"; required $Expected"}else{''})
}
$specs = @(Get-ChildItem (Join-Path $PSScriptRoot '../spec') -Recurse -Filter *.tla | Sort-Object FullName)
foreach ($name in $Model) {
    if ($name -notin $specs.BaseName) { throw "Unknown model: $name" }
}
foreach ($spec in $specs | Where-Object { $Model.Count -eq 0 -or $_.BaseName -in $Model }) {
    $configuration = Get-Content -LiteralPath (Join-Path $spec.DirectoryName ($spec.BaseName + '.cfg')) -Raw
    Check-Model $spec 'correct' $configuration ''
    $controls = Join-Path $spec.DirectoryName ($spec.BaseName + '.negative.json')
    if (-not (Test-Path -LiteralPath $controls)) { throw "Model has no declared negative control: $($spec.FullName)" }
    foreach ($control in @(Get-Content -LiteralPath $controls -Raw | ConvertFrom-Json)) {
        $faulty = $configuration
        foreach ($property in $control.overrides.PSObject.Properties) {
            $key = [regex]::Escape($property.Name)
            if ($property.Value -is [bool]) {
                $pattern = '(?m)(\b' + $key + '\s*=\s*)(TRUE|FALSE)\b'
                $replacement = if ($property.Value) { 'TRUE' } else { 'FALSE' }
            } elseif ($property.Value -is [string] -and $property.Value -cmatch '^[A-Za-z][A-Za-z0-9]*$') {
                $pattern = '(?m)(\b' + $key + '\s*=\s*)"None"'
                $replacement = '"' + $property.Value + '"'
            } else { throw "Invalid fault value for $($property.Name)" }
            if ([regex]::Matches($faulty,$pattern).Count -ne 1) { throw "Expected one control override for $($property.Name) in $($spec.Name)" }
            $faulty = [regex]::Replace($faulty,$pattern,('`${1}' + $replacement).Replace('`',''))
        }
        Check-Model $spec $control.name $faulty $control.expected
    }
}
$results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Evidence 'model-controls.json')
if ($results.pass -contains $false) { exit 1 }
exit 0
