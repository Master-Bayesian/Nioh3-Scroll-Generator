[CmdletBinding()]
param(
    [string]$Python,
    [string]$LuaLibrary,
    [string[]]$TestPath = @('tests'),
    [string[]]$PytestArgument = @('-q'),
    # Run one project Python script in the same prepared environment. This keeps
    # evidence/package tools off ambient Python without duplicating the resolver.
    [string]$ScriptPath,
    [string[]]$ScriptArgument = @(),
    # Cargo has no cache quota or automatic eviction. Fail before a heavy run
    # can fill the build volume; 0 is an explicit lightweight-run override.
    [double]$MinimumFreeGiB = 5.0,
    # Resolve and create the build root, report the environment this run uses
    # and exit before Python starts. tests/migration/test_build_root_policy.py
    # drives it to prove this runner and tests/migration/cargo_target.py agree.
    [switch]$PrintEnvironment
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

# One build/test root per host, mirroring tests/migration/cargo_target.py:
# an explicit NIOH3_BUILD_ROOT wins, a local Windows host uses the D: delivery
# volume, and CI, other hosts or a Windows host without a D: drive keep the
# platform temp directory. Explicit CARGO_TARGET_DIR still wins for Cargo. No
# build or test temp may be written to the C: system temp on this host.
$localBuildRoot = 'D:\Nioh3_v080_deliverables'
# Must match PYTHON_TEST_CACHE_NAME in tests/migration/cargo_target.py.
$cargoCacheName = 'python-tests'

function Get-BuildRoot {
    $configured = $env:NIOH3_BUILD_ROOT
    if (-not [string]::IsNullOrWhiteSpace($configured)) {
        return [pscustomobject]@{ Path = [System.IO.Path]::GetFullPath($configured); Dedicated = $true }
    }
    $isCi = (-not [string]::IsNullOrWhiteSpace($env:CI)) -or
        (-not [string]::IsNullOrWhiteSpace($env:GITHUB_ACTIONS))
    $isWindowsHost = if ($null -eq $IsWindows) { $env:OS -eq 'Windows_NT' } else { [bool]$IsWindows }
    if ($isWindowsHost -and -not $isCi) {
        $driveRoot = [System.IO.Path]::GetPathRoot($localBuildRoot)
        if (-not [string]::IsNullOrWhiteSpace($driveRoot) -and
            (Test-Path -LiteralPath $driveRoot -PathType Container)) {
            return [pscustomobject]@{ Path = $localBuildRoot; Dedicated = $true }
        }
    }
    $tempPath = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    return [pscustomobject]@{
        Path      = $tempPath.TrimEnd([char[]]@('\', '/'))
        Dedicated = $false
    }
}

function Get-CargoTargetDirectory {
    param([pscustomobject]$RootInfo)

    $configured = $env:CARGO_TARGET_DIR
    if (-not [string]::IsNullOrWhiteSpace($configured)) {
        return [System.IO.Path]::GetFullPath($configured)
    }
    if ($RootInfo.Dedicated) {
        return (Join-Path (Join-Path $RootInfo.Path 'build-cache') $cargoCacheName)
    }
    return (Join-Path $RootInfo.Path ('nioh3-' + $cargoCacheName + '-target'))
}

function Assert-BuildVolumeCapacity {
    param(
        [pscustomobject]$RootInfo,
        [double]$RequiredGiB
    )

    if ($RequiredGiB -lt 0) {
        throw '-MinimumFreeGiB cannot be negative.'
    }
    if ($RequiredGiB -eq 0) {
        return $null
    }
    $fullRoot = [System.IO.Path]::GetFullPath($RootInfo.Path)
    $volumeRoot = [System.IO.Path]::GetPathRoot($fullRoot)
    if ([string]::IsNullOrWhiteSpace($volumeRoot)) {
        throw "Cannot resolve the build volume for $fullRoot."
    }
    try {
        $drive = [System.IO.DriveInfo]::new($volumeRoot)
        $availableBytes = [int64]$drive.AvailableFreeSpace
    } catch {
        throw "Cannot read free space for build volume $volumeRoot`: $($_.Exception.Message)"
    }
    $requiredBytes = [int64][math]::Ceiling($RequiredGiB * 1GB)
    if ($availableBytes -lt $requiredBytes) {
        $availableGiB = [math]::Round($availableBytes / 1GB, 2)
        throw @"
NIOH3_BUILD_VOLUME_LOW_SPACE: $volumeRoot has $availableGiB GiB free; this run requires at least $RequiredGiB GiB.
Reuse the shared Cargo target or run `cargo clean --target-dir <explicit-disposable-target>` on a verified task-specific cache.
Do not delete source, deliverables, the shared target, or unrelated caches. Use -MinimumFreeGiB 0 only for a proven lightweight command.
"@
    }
    return $availableBytes
}

function Resolve-ProjectPython {
    param([string]$RequestedPython)

    $candidates = [System.Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($RequestedPython)) {
        $candidates.Add($RequestedPython)
    } elseif (-not [string]::IsNullOrWhiteSpace($env:NIOH3_PYTHON)) {
        $candidates.Add($env:NIOH3_PYTHON)
    } else {
        $candidates.Add((Join-Path $projectRoot '.codex_tmp/v2-build-env/Scripts/python.exe'))
        $candidates.Add((Join-Path $projectRoot '.venv/Scripts/python.exe'))
    }

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    throw @"
No prepared project Python environment was found.
Create .codex_tmp/v2-build-env and install requirements-dev.txt, or pass
-Python / NIOH3_PYTHON with an environment containing the tracked dependencies.
"@
}

# The build root, the shared Cargo cache and this run's isolated test temp are
# created before Python starts, so every child process (pytest, cargo, rustc,
# PyInstaller) inherits them. The per-run temp directory is unique, so parallel
# runs never share the temp tree Cargo and pytest lock; the Cargo target is
# deliberately shared and stays warm.
$rootInfo = Get-BuildRoot
$cargoTargetDirectory = Get-CargoTargetDirectory -RootInfo $rootInfo
$tempRoot = if ($rootInfo.Dedicated) {
    Join-Path $rootInfo.Path 'tmp'
} else {
    $rootInfo.Path
}
$freeBytes = if ($PrintEnvironment) {
    $null
} else {
    Assert-BuildVolumeCapacity -RootInfo $rootInfo -RequiredGiB $MinimumFreeGiB
}
$runTemp = Join-Path $tempRoot ('pytest-' + $PID + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
foreach ($directory in @($rootInfo.Path, $cargoTargetDirectory, $tempRoot, $runTemp)) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
}

$savedEnvironment = @{
    TEMP             = $env:TEMP
    TMP              = $env:TMP
    CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR
}

$exitCode = 1
try {
    $env:TEMP = $runTemp
    $env:TMP = $runTemp
    $env:CARGO_TARGET_DIR = $cargoTargetDirectory

    if ($PrintEnvironment) {
        Write-Output ('NIOH3_ENV ROOT=' + $rootInfo.Path)
        Write-Output ('NIOH3_ENV CARGO_TARGET=' + $cargoTargetDirectory)
        Write-Output ('NIOH3_ENV TEMP_ROOT=' + $tempRoot)
        Write-Output ('NIOH3_ENV RUN_TEMP=' + $runTemp)
        $exitCode = 0
    } else {
        $pythonExecutable = Resolve-ProjectPython -RequestedPython $Python
        & $pythonExecutable -c 'import pytest, lupa'
        if ($LASTEXITCODE -ne 0) {
            throw "The selected Python environment is incomplete: $pythonExecutable. Install requirements-dev.txt."
        }

        if ([string]::IsNullOrWhiteSpace($LuaLibrary) -and
            [string]::IsNullOrWhiteSpace($env:LUA54_LIBRARY)) {
            $ceLuaLibrary = 'C:\Program Files\Cheat Engine\lua53-64.dll'
            if (Test-Path -LiteralPath $ceLuaLibrary -PathType Leaf) {
                $LuaLibrary = $ceLuaLibrary
            }
        }

        if (-not [string]::IsNullOrWhiteSpace($LuaLibrary)) {
            if (-not (Test-Path -LiteralPath $LuaLibrary -PathType Leaf)) {
                throw "The requested Lua shared library does not exist: $LuaLibrary"
            }
            $env:LUA54_LIBRARY = (Resolve-Path -LiteralPath $LuaLibrary).Path
        }

        Write-Host "Project Python: $pythonExecutable"
        Write-Host "Build root: $($rootInfo.Path)"
        Write-Host "Cargo target: $cargoTargetDirectory"
        Write-Host "Test temp: $runTemp"
        if ($null -ne $freeBytes) {
            Write-Host ('Build-volume free space: ' + [math]::Round($freeBytes / 1GB, 2) + ' GiB')
        }
        if (-not [string]::IsNullOrWhiteSpace($env:LUA54_LIBRARY)) {
            Write-Host "Lua test library: $env:LUA54_LIBRARY"
        }

        if (-not [string]::IsNullOrWhiteSpace($ScriptPath)) {
            $candidate = if ([System.IO.Path]::IsPathRooted($ScriptPath)) {
                $ScriptPath
            } else {
                Join-Path $projectRoot $ScriptPath
            }
            if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
                throw "The requested project Python script does not exist: $candidate"
            }
            $resolvedScript = (Resolve-Path -LiteralPath $candidate).Path
            & $pythonExecutable $resolvedScript @ScriptArgument
        } else {
            $pytestArguments = @('-m', 'pytest') + $PytestArgument + $TestPath
            & $pythonExecutable @pytestArguments
        }
        $exitCode = $LASTEXITCODE
    }
} finally {
    # The caller's environment is never left pointing at this run's directories.
    $env:TEMP = $savedEnvironment.TEMP
    $env:TMP = $savedEnvironment.TMP
    if ([string]::IsNullOrEmpty($savedEnvironment.CARGO_TARGET_DIR)) {
        Remove-Item -Path 'Env:CARGO_TARGET_DIR' -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_TARGET_DIR = $savedEnvironment.CARGO_TARGET_DIR
    }
    # A failed run keeps this run's temp tree for inspection.
    if ($exitCode -eq 0 -and
        $runTemp.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and
        (Test-Path -LiteralPath $runTemp -PathType Container)) {
        Remove-Item -LiteralPath $runTemp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

exit $exitCode
