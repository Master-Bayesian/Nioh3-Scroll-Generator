param([string]$Name, [int]$Take = 60, [string]$Merged = 'merged2', [string]$Set = '', [string]$Pieces = '3', [int]$Rank = 0, [int]$Top = 30, [string]$Tags = 'own,ee,hc,yy,yi')
# Search each save with the kernel in $Name. With -Set, only that save's pruned required-set space; otherwise pruned, everything. Collect misses.
$c='F:/Nioh3_r16_build/work/compile'; $o="$c/$Name"; $env:NIOH3_BUILD_ROOT='F:/Nioh3_r16_build'
$sp='F:/Nioh3_r16_build/worktrees/research-damage/research/damage/evaluator/src/pool_prune.py'
$spaces = Get-Content "$c/$Merged/spaces.json" -Raw | ConvertFrom-Json
$all = @(); $tops = @()
foreach ($tag in $Tags.Split(',')) {
    $s = $spaces.$tag; $d = "$c/f_$tag"; New-Item -ItemType Directory -Force $d | Out-Null
    Remove-Item "$d/*" -Force -ErrorAction SilentlyContinue
    foreach ($n in 'hits.txt','path_kernel.exe','arena.bin','mapped.bin','tables.bin','frame.bin','characters.bin','dom_base.bin','dom_known.bin','gpu_cases.txt','pool.bin','live_groups.txt','survival_groups.txt') { Copy-Item "$o/$n" "$d/$n" -Force }
    $lines = [System.Collections.ArrayList]@(Get-Content "$o/gpu_setup.txt"); $opn = [int]$lines[0].Split(' ')[2]; $n = [int]$lines[1 + $opn]
    $slots = (Get-Content "$c/$Name/gpu_setup.txt"); $keep = @($s.PSObject.Properties | Where-Object { $_.Value[1] -gt 0 -and (-not $env:NIOH3_WEAPON_POOL -or $_.Name -ne '0') } | ForEach-Object { "$($_.Name) $($_.Value[0]) $($_.Value[1])" })
    $tail = $lines[2 + $opn + $n]
    $head = @($lines[0..$opn]); ($head + @("$($keep.Count)") + $keep + @($tail)) | Set-Content "$d/gpu_setup.txt" -Encoding ascii
    $q = "${d}_p"
    if ($Set -eq 'top') {
        $list = & 'F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1' -MinimumFreeGiB 0 -ScriptPath $sp -ScriptArgument @($d,$q,'list') 2>&1 | Where-Object { "$_" -match '^0x' } | Select-Object -Skip $Rank -First 1
        $key = "$list".Split(' ')[0]
        $o2 = & 'F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1' -MinimumFreeGiB 0 -ScriptPath $sp -ScriptArgument @($d,$q,$key,$Pieces) 2>&1 | Where-Object { "$_" -match '^spaces' }
        "$tag set $key at least $Pieces`: $o2"
    } else {
        $o2 = & 'F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1' -MinimumFreeGiB 0 -ScriptPath $sp -ScriptArgument @($d,$q) 2>&1 | Where-Object { "$_" -match '^combinations' }
        "$tag $o2"
    }
    $text = & "$q/path_kernel.exe" $q 0.97 2>&1 | Where-Object { "$_" -match 'searched|polish|points moved' }
    "  $($text -join ' | ')"
    Get-Content "$q/gpu_facts.txt" -TotalCount 1 -ErrorAction SilentlyContinue
    if (Test-Path "$q/gpu_top.txt") {
        $origin = (Get-Content "$q/gpu_origin.txt" -Raw).Trim().Split(' ')
        foreach ($line in (Get-Content "$q/gpu_top.txt" -TotalCount $Top)) {
            $words = $line.Split(' ')
            for ($k = 1; $k -lt $words.Count; $k++) { if ($words[$k] -match '^(\d+):(\d+)$') { $words[$k] = "$($Matches[1]):$($origin[[int]$Matches[2]])" } }
            $tops += ($words -join ' ')
        }
    }
    if (Test-Path "$q/gpu_nopath.txt") {
        $miss = Get-Content "$q/gpu_nopath.txt"
        $step = [Math]::Max(1, [int]($miss.Count / $Take))
        $origin = (Get-Content "$q/gpu_origin.txt" -Raw).Trim().Split(' ')
        for ($i = 0; $i -lt $miss.Count; $i += $step) {
            $words = $miss[$i].Split(' ')
            for ($k = 1; $k -lt $words.Count; $k++) { if ($words[$k] -match '^(\d+):(\d+)$') { $words[$k] = "$($Matches[1]):$($origin[[int]$Matches[2]])" } }
            $all += ($words -join ' ')
        }
    }
}
$all | Set-Content "$o/miss_all.txt" -Encoding ascii
$tops | Set-Content "$o/top_all.txt" -Encoding ascii
"collected $($all.Count)"
