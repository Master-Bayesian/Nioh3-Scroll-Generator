param([int]$WeaponType, [int]$Move = 25036, [int]$Stance = 1, [string]$Key = '', [string]$Tags = 'own', [int]$Rounds = 3, [int]$Show = 10)
# One optimizer run: search outfits for one move of one weapon and print the best results.
$c = 'F:/Nioh3_r16_build/work/compile'
$tag = "opt$WeaponType"
pwsh -NoProfile -File "$PSScriptRoot/move_pair.ps1" -Tag $tag -Move "${WeaponType}:${Move}:$Stance" -Key $Key -Rounds $Rounds -Tags $Tags
Get-Content "$c/gpu_${tag}_$Rounds/top_all.txt" -TotalCount $Show
