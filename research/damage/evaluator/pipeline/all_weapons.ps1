$c='F:/Nioh3_r16_build/work/compile'
$moves = Get-Content 'F:/Nioh3_r16_build/worktrees/research-damage/research/damage/data/MOVES.json' -Raw | ConvertFrom-Json
foreach ($w in $moves) {
    if ($w.type -in 28275, 29361, 15406) { continue }
    $move = $w.moves | Where-Object { $_.attack_class -eq 1 -and $_.hits.Count -gt 0 } | Sort-Object id | Select-Object -First 1
    if (-not $move) { "skip $($w.type): no light move"; continue }
    "##### $($w.type) $($w.label) move $($move.id) hits $($move.hits.Count)"
    pwsh -NoProfile -File "$c/move_pair.ps1" -Tag "w$($w.type)" -Move "$($w.type):$($move.id):1" -Key '' *>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match '^===|^candidates|GPU claims|^cases|searched|^collected|no weapon of type' }
}
"##### katana regression"
pwsh -NoProfile -File "$c/move_pair.ps1" -Tag ka -Move '6409:25036:1' -Key katana *>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match '^===|^candidates|GPU claims|^cases|searched|^collected' }
"##### done"
