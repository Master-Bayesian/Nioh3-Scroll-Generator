param([string]$Python='python')
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$output=Join-Path $root '.codex_tmp/native-fault-ci'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$locator=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installation=& $locator -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if(-not $installation){throw 'MSVC x64 build tools are required'}
$developer=Join-Path $installation 'Common7/Tools/VsDevCmd.bat'
$source=Join-Path $root 'research/native_dispatch_fixture.cpp'
# Fixed, scoped compiler paths; no file deletion or moving across shells.
$command='call "'+$developer+'" -no_logo -arch=x64 && cl /nologo /EHsc /O2 "'+$source+'" /Fe:"'+(Join-Path $output 'native_dispatch_fixture.exe')+'" /Fo:"'+(Join-Path $output 'native_dispatch_fixture.obj')+'"'
& cmd.exe /d /s /c $command
if($LASTEXITCODE -ne 0){throw 'Synthetic fixture compilation failed'}
& $Python (Join-Path $root 'research/test_native_dispatch_fixture.py') $output
if($LASTEXITCODE -ne 0){throw 'Native dispatch baseline failed'}
& $Python (Join-Path $root 'research/verify_native_fault_matrix.py') $output
if($LASTEXITCODE -ne 0){throw 'Native failure matrix failed'}
