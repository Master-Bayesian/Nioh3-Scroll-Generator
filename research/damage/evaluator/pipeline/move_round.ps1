param([string]$Tag, [string]$Move, [string]$Key = '', [int]$Count = 60, [string]$Set = 'top', [string]$Pieces = '3', [string]$Tags = 'own')
$c='F:/Nioh3_r16_build/work/compile'; $env:NIOH3_BUILD_ROOT='F:/Nioh3_r16_build'; $env:PYTHONIOENCODING='utf-8'
$Forced = @(($env:NIOH3_FORCED + "").Split(";") | Where-Object { $_ })
$d='F:/Nioh3_r16_build/worktrees/research-damage/research/damage'
$env:NIOH3_MOVE=$Move
if ($Key -ne 'katana') { $env:NIOH3_WEAPON_POOL='F:/Nioh3_r16_build/work/live-1107/pool_f0.bin'; $env:NIOH3_WEAPON_KEY=$Key }
$e="$c/export_$Tag"; $g="gpu_$Tag"
New-Item -ItemType Directory -Force "$c/$g" | Out-Null
Copy-Item "$c/build_template.cmd" "$c/$g/build.cmd" -Force
& 'F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1' -MinimumFreeGiB 0 -ScriptPath "$d/evaluator/src/path_outfits.py" -ScriptArgument (@("$d/data/SOURCE_LAYERS.json","$c/merged2","$c/outfits_$Tag.json","$Count",$e) + $Forced) 2>&1 | ForEach-Object { "$_" } | Tee-Object "$c/export_$Tag.log" | Where-Object { $_ -match '^candidates|GPU claims|exported|disagree|stopped|refused|Traceback|Error' } | Select-Object -Last 14
New-Item -ItemType File -Force "$e/touched.txt" | Out-Null
pwsh -NoProfile -File "$c/gpu_cycle.ps1" -Name $g -Export "export_$Tag" -Runs '0.97 c' 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match '^generated|^cases|BUILD|error' }
pwsh -NoProfile -File "$c/coverage.ps1" -Name $g -Set $Set -Pieces $Pieces -Tags $Tags 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match 'searched|^collected|set 0x|facts' }
