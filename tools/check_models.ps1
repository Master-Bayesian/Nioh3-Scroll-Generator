#Requires -Version 7
# Check the real models and require named counterexamples from each declared faulty variant.
param([string]$Evidence = $env:NIOH3_QUALITY_EVIDENCE, [string]$Java = 'java', [string[]]$Model = @())
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$version = 'v1.8.0'
# Official v1.8.0 asset 768714348, verified against GitHub's asset metadata
# and downloaded bytes after the 2026-11-19 replacement (4,510,915 bytes).
$sha256 = 'FDFFEF247F563EF6EDBFD44958D2A82DB79E7564E07CB673262D896353FA6713'
$root = if ($env:NIOH3_BUILD_ROOT) { $env:NIOH3_BUILD_ROOT } elseif ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP 'nioh3-quality' } else { 'D:/Nioh3_v080_deliverables' }
$cache = Join-Path $root 'tools/tla'
if (-not $Evidence) { $Evidence = Join-Path $root 'deliverables/quality-controls' }
New-Item -ItemType Directory -Force $cache,$Evidence | Out-Null
$jar = Join-Path $cache "tla2tools-$sha256.jar"
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
            $pattern = '(?m)(\b' + [regex]::Escape($property.Name) + '\s*=\s*)(TRUE|FALSE)\b'
            if (-not [regex]::IsMatch($faulty,$pattern)) { throw "Control override missing: $($property.Name) in $($spec.Name)" }
            $replacement = if ($property.Value) { 'TRUE' } else { 'FALSE' }
            $faulty = [regex]::Replace($faulty,$pattern,('`${1}' + $replacement).Replace('`',''))
        }
        Check-Model $spec $control.name $faulty $control.expected
    }
}
$results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Evidence 'model-controls.json')
if ($results.pass -contains $false) { exit 1 }
exit 0
