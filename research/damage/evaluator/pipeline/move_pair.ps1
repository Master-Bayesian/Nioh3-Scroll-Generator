param([string]$Tag, [string]$Move, [string]$Key, [int]$Rounds = 3, [string]$Tags = 'own', [int]$Count = 40)
$c='F:/Nioh3_r16_build/work/compile'
$forced=@()
for ($r=1; $r -le $Rounds; $r++) {
    "=== $Tag round $r"
    $env:NIOH3_FORCED = $forced -join ';'
    pwsh -NoProfile -File "$c/move_round.ps1" -Tag "${Tag}_$r" -Move $Move -Key $Key -Count $Count -Tags $Tags *>&1 | ForEach-Object { "$_".Substring(0,[math]::Min(260,"$_".Length)) } | Where-Object { "$_" -notmatch '^(Project Python|Build root|Cargo target|Test temp|Lua test)' }
    $g="$c/gpu_${Tag}_$r"
    $f="$c/forced_${Tag}_$r.txt"
    (@(Get-Content "$g/miss_all.txt" -ErrorAction SilentlyContinue) + @(Get-Content "$g/top_all.txt" -TotalCount 8 -ErrorAction SilentlyContinue)) | Where-Object { $_ } | Set-Content $f -Encoding ascii
    $forced += $f
}
