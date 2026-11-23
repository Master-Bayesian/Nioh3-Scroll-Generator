#Requires -Version 7
param(
    [Parameter(Mandatory)][string]$Baseline,
    [Parameter(Mandatory)][string]$Translator,
    [Parameter(Mandatory)][string]$Native,
    [UInt64]$SourceOwner,
    [string[]]$AdditionalDomain = @(),
    [UInt64]$SkillKeyAddress,
    [Parameter(Mandatory)][string]$Python,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$scene = Get-Content "$Baseline/scene.txt" -Raw
if ($SkillKeyAddress) {
    # The translated ABI accepts four-byte overlays. The skill producer owns
    # two bytes. Keep the adjacent halfword in the immutable knownness image.
    $preserve = $SkillKeyAddress + 2
    $ranges = [regex]::Matches($scene, '(?m)^domain ([0-9a-f]+) ([0-9a-f]+)\r?$')
    $matched = 0
    foreach ($range in $ranges) {
        $start = [Convert]::ToUInt64($range.Groups[1].Value,16)
        $end = [Convert]::ToUInt64($range.Groups[2].Value,16)
        if ($start -le $SkillKeyAddress -and $preserve+2 -lt $end) {
            $split = "domain {0:x} {1:x}`ndomain {2:x} {3:x}" -f $start,$preserve,($preserve+2),$end
            $scene = $scene.Replace($range.Value,$split)
            $matched++
        }
    }
    if ($matched -ne 1) { throw 'Skill key has no unique input domain' }
}
$spec = Get-Content "$Baseline/inputs.json" -Raw | ConvertFrom-Json
$dir = (New-Item -ItemType Directory -Path $Output).FullName
$regions = foreach ($range in $AdditionalDomain) {
    if ($range -notmatch '^([0-9a-fA-F]+):([0-9a-fA-F]+)$') { throw "Invalid domain $range" }
    $lo = [Convert]::ToUInt64($Matches[1], 16)
    $hi = [Convert]::ToUInt64($Matches[2], 16)
    if ($hi -le $lo) { throw "Empty domain $range" }
    "domain {0:x} {1:x}" -f $lo,$hi
}
if ($SourceOwner) { $regions = @($regions) + ("domain {0:x} {1:x}" -f ($SourceOwner+0x2e3c),($SourceOwner+0x2e40)) }
if (-not $regions) { throw 'At least one additional input range is required' }
[IO.File]::WriteAllText("$dir/scene.txt", $scene.TrimEnd() + "`n" + ($regions -join "`n") + "`n")
Copy-Item -LiteralPath "$Baseline/main_windows.h" -Destination $dir
$vs = 'C:/Program Files/Microsoft Visual Studio/18/Community'
Import-Module "$vs/Common7/Tools/Microsoft.VisualStudio.DevShell.dll"
Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=amd64 -host_arch=amd64'
Push-Location $dir
try {
    & $Python $Translator $spec.listing $spec.image "$dir/scene.txt" "$dir/main_gen.inc" 400 --stack-forward --stack-window *> source.log
    if ($LASTEXITCODE) { throw "Translation failed. See $dir/source.log" }
    & cl.exe /nologo /TP /std:c++17 /LD /O2 /MT /fp:strict /EHsc /bigobj /wd4819 /DME_CPU_ONLY "/I$dir" "/I$Native" "$Native/e2e/e2e_main.cu" /link /OUT:main_cpu.dll *> cpu-build.log
    if ($LASTEXITCODE) { throw "CPU build failed. See $dir/cpu-build.log" }
    $nvcc = 'C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.3/bin/nvcc.exe'
    & $nvcc -shared -std=c++17 -arch=sm_89 -O2 --fmad=false --ftz=false -DRT_LINES=16 -Xcompiler /MT -Xcompiler /bigobj -Xcompiler /wd4819 "-I$dir" "-I$Native" "$Native/e2e/e2e_main.cu" -o main_device.dll *> cuda-build.log
    if ($LASTEXITCODE) { throw "CUDA build failed. See $dir/cuda-build.log" }
} finally {
    Pop-Location
}
