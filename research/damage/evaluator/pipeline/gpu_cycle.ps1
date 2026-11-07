param([string]$Name = 'gpu2', [string[]]$Runs = @('0.97'), [string]$Export = 'export4')
$env:NIOH3_BUILD_ROOT='F:/Nioh3_r16_build'; $env:PYTHONIOENCODING='utf-8'
$d='F:/Nioh3_r16_build/worktrees/research-damage/research/damage'; $o="F:/Nioh3_r16_build/work/compile/$Name"
& 'F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1' -MinimumFreeGiB 0 -ScriptPath "$d/evaluator/src/path_cuda.py" -ScriptArgument @("F:/Nioh3_r16_build/work/compile/$Export",$o) 2>&1 | Select-Object -Last 1
Set-Location $o
Remove-Item path_kernel.exe -ErrorAction SilentlyContinue
& cmd.exe /c "$($o -replace '/','\')\build.cmd" 2>&1 | Where-Object { $_ -match ' error|error:' -and $_ -notmatch 'C4819' } | Select-Object -Last 12
if (-not (Test-Path path_kernel.exe)) { 'BUILD FAILED'; exit 1 }
foreach ($r in $Runs) {
    $p = Start-Process -FilePath .\path_kernel.exe -ArgumentList "$o $r" -NoNewWindow -PassThru -RedirectStandardOutput run.txt -RedirectStandardError err.txt
    if (-not $p.WaitForExit(400000)) { $p.Kill(); 'killed' }
    "--- $r"; Get-Content run.txt | Where-Object { $_ -notmatch ' of ' }; Get-Content err.txt -Tail 3
}

