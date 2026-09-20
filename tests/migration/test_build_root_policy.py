"""Policy gate for the project build root: D-backed locally, portable elsewhere.

The failure this locks down: on a host whose delivery volume is `D:`, a build or
test helper quietly used the `C:` system temp, so Cargo targets, pytest temp and
every Rust/Python child temp competed with the OS for the system drive.

The rule lives in two places only — `tests/migration/cargo_target.py` and
`tools/run_python_tests.ps1` — and both are driven here with the same inputs so
they cannot drift: an explicit `NIOH3_BUILD_ROOT` (or `CARGO_TARGET_DIR`) wins,
a local Windows host with a D: drive builds on `D:/Nioh3_v080_deliverables`, and
CI, non-Windows hosts and Windows hosts without that drive keep the platform
temp directory. Nothing here writes a large artifact: the resolver tests use a
temporary directory as the stand-in delivery volume, and the runner checks use
`-PrintEnvironment`.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.migration import cargo_target  # noqa: E402
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402


RUNNER = ROOT / "tools" / "run_python_tests.ps1"
CARGO_CACHE_NAME = cargo_target.PYTHON_TEST_CACHE_NAME
PWSH = shutil.which("pwsh")
requires_pwsh = pytest.mark.skipif(PWSH is None, reason="PowerShell 7 runs the project test runner")


@pytest.fixture(autouse=True)
def _platform_temp_matches_the_runner(monkeypatch):
    """Make the in-process temp root follow TEMP/TMP exactly like the runner.

    Python prefers `TMPDIR` and .NET does not, so the cached Python answer is
    dropped and `TMPDIR` is removed; both resolvers then read the same variables.
    """

    monkeypatch.delenv("TMPDIR", raising=False)
    monkeypatch.setattr(tempfile, "tempdir", None)


def same_path(left, right) -> bool:
    return os.path.normcase(os.path.normpath(str(left))) == os.path.normcase(os.path.normpath(str(right)))


def _quoted(path) -> str:
    """A path as a single-quoted PowerShell literal."""

    return str(path).replace("'", "''")


def project_python() -> str | None:
    """The same candidates the runner resolves, so a skip means really absent."""

    candidates = (
        os.environ.get("NIOH3_PYTHON", "").strip(),
        str(ROOT / ".codex_tmp/v2-build-env/Scripts/python.exe"),
        str(ROOT / ".venv/Scripts/python.exe"),
    )
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return candidate
    return None


def runner_environment(overrides: dict) -> dict:
    """`-PrintEnvironment` output under a controlled environment inheritance."""

    environment = dict(os.environ)
    for name in ("NIOH3_BUILD_ROOT", "CARGO_TARGET_DIR", "TMPDIR"):
        environment.pop(name, None)
    environment.update(overrides)
    result = subprocess.run(
        [str(PWSH), "-NoLogo", "-NoProfile", "-File", str(RUNNER), "-PrintEnvironment"],
        cwd=str(ROOT),
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
    )
    assert result.returncode == 0, f"the runner failed:\n{result.stdout}\n{result.stderr}"
    values = {}
    for line in result.stdout.splitlines():
        if not line.startswith("NIOH3_ENV "):
            continue
        name, separator, value = line[len("NIOH3_ENV ") :].partition("=")
        assert separator, f"unparseable runner line: {line}"
        values[name.strip()] = value.strip()
    for expected in ("ROOT", "CARGO_TARGET", "TEMP_ROOT", "RUN_TEMP"):
        assert expected in values, f"the runner must report {expected}:\n{result.stdout}"
    return values


def test_a_local_windows_host_resolves_the_d_backed_root(monkeypatch, tmp_path) -> None:
    volume = tmp_path / "delivery volume"
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.setattr(cargo_target, "LOCAL_BUILD_ROOT", volume)
    monkeypatch.setattr(cargo_target, "_host_is_local_windows", lambda local_root: True)

    assert cargo_target.build_root() == volume
    assert cargo_target.temp_root() == volume / "tmp"
    assert cargo_target.cargo_target_dir("m2-effect-parity") == volume / "build-cache" / "m2-effect-parity"
    assert Path(resolved_cargo_target_dir("policy-probe")) == volume / "build-cache" / "policy-probe"
    assert (volume / "build-cache" / "policy-probe").is_dir(), "the resolver creates the root it returns"
    assert (volume / "tmp").is_dir()
    assert not same_path(volume, Path(tempfile.gettempdir()))


def test_this_host_builds_on_the_delivery_volume(monkeypatch) -> None:
    """The owner's rule, on the owner's machine, with no override in play."""

    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    if os.name != "nt":
        pytest.skip("the D-backed root is a Windows host rule")
    if os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
        pytest.skip("CI keeps its platform temp root")
    if not Path("D:/").exists():
        pytest.skip("this host has no D: delivery volume")

    gate_target = cargo_target.LOCAL_BUILD_ROOT / "build-cache" / "migration"
    assert cargo_target.build_root() == cargo_target.LOCAL_BUILD_ROOT
    assert not same_path(cargo_target.temp_root(), Path(tempfile.gettempdir()))
    assert Path(resolved_cargo_target_dir()) == gate_target
    assert gate_target.is_dir(), "the default gate target is created on the delivery volume"


def test_an_explicit_build_root_wins_everywhere(monkeypatch, tmp_path) -> None:
    probe = tmp_path / "explicit build root"
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.setenv("NIOH3_BUILD_ROOT", str(probe))
    monkeypatch.setenv("CI", "true")
    monkeypatch.setattr(cargo_target, "_host_is_local_windows", lambda local_root: True)

    assert cargo_target.build_root() == probe
    assert cargo_target.temp_root() == probe / "tmp"
    assert cargo_target.cargo_target_dir("probe") == probe / "build-cache" / "probe"

    cargo = tmp_path / "explicit cargo target"
    monkeypatch.setenv("CARGO_TARGET_DIR", str(cargo))
    assert cargo_target.cargo_target_dir("probe") == cargo
    assert cargo.is_dir(), "an explicit Cargo target is created, never silently replaced"


def test_a_portable_host_keeps_the_platform_temp_root(monkeypatch) -> None:
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.setattr(cargo_target, "_host_is_local_windows", lambda local_root: False)

    assert cargo_target.build_root() == Path(tempfile.gettempdir())
    assert cargo_target.temp_root() == Path(tempfile.gettempdir())
    assert cargo_target.cargo_target_dir("probe") == Path(tempfile.gettempdir()) / "nioh3-probe-target"


def test_ci_declines_the_local_volume_even_though_the_drive_exists(monkeypatch) -> None:
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.setenv("CI", "true")

    assert cargo_target.build_root() == Path(tempfile.gettempdir())
    if os.name == "nt" and Path("D:/").exists():
        assert not same_path(cargo_target.build_root(), cargo_target.LOCAL_BUILD_ROOT)
        assert cargo_target._host_is_local_windows(cargo_target.LOCAL_BUILD_ROOT) is False


def test_an_unusable_d_backed_root_fails_closed_instead_of_using_the_c_temp(monkeypatch, tmp_path) -> None:
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.setattr(cargo_target, "LOCAL_BUILD_ROOT", tmp_path / "blocked")
    monkeypatch.setattr(cargo_target, "_host_is_local_windows", lambda local_root: True)

    def refuse(self, *args, **kwargs):
        raise PermissionError("refused by the policy probe")

    monkeypatch.setattr(Path, "mkdir", refuse)
    with pytest.raises(RuntimeError) as failure:
        cargo_target.build_root()
    assert "NIOH3_BUILD_ROOT" in str(failure.value)
    with pytest.raises(RuntimeError):
        cargo_target.cargo_target_dir("probe")


@requires_pwsh
def test_the_runner_and_the_resolver_agree_on_this_host(monkeypatch) -> None:
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)

    values = runner_environment({})
    assert same_path(values["ROOT"], cargo_target.build_root())
    assert same_path(values["CARGO_TARGET"], cargo_target.cargo_target_dir(CARGO_CACHE_NAME))
    assert same_path(values["TEMP_ROOT"], cargo_target.temp_root())
    assert Path(values["RUN_TEMP"]).parent == Path(values["TEMP_ROOT"])
    assert not Path(values["RUN_TEMP"]).exists(), "a clean run removes its own temp tree"


@requires_pwsh
def test_the_runner_and_the_resolver_agree_under_an_explicit_root_with_spaces(tmp_path) -> None:
    probe = tmp_path / "delivery volume created by runner"
    values = runner_environment({"NIOH3_BUILD_ROOT": str(probe)})

    assert same_path(values["ROOT"], probe)
    assert same_path(values["CARGO_TARGET"], probe / "build-cache" / CARGO_CACHE_NAME)
    assert same_path(values["TEMP_ROOT"], probe / "tmp")
    assert probe.is_dir(), "the runner creates the root it resolves"
    assert Path(values["TEMP_ROOT"]).is_dir()


@requires_pwsh
def test_the_runner_and_the_resolver_agree_on_a_ci_host(monkeypatch) -> None:
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.setenv("CI", "true")

    values = runner_environment({"CI": "true"})
    assert same_path(values["ROOT"], cargo_target.build_root())
    assert Path(values["ROOT"]) == Path(tempfile.gettempdir())
    assert same_path(values["CARGO_TARGET"], cargo_target.cargo_target_dir(CARGO_CACHE_NAME))
    if os.name == "nt":
        assert not same_path(values["ROOT"], cargo_target.LOCAL_BUILD_ROOT)


@requires_pwsh
def test_the_runner_restores_the_callers_environment(tmp_path) -> None:
    sentinel = tmp_path / "sentinel temp"
    script = ";".join(
        (
            "Remove-Item -Path 'Env:CARGO_TARGET_DIR' -ErrorAction SilentlyContinue",
            f"$env:TEMP='{sentinel}'",
            f"$env:TMP='{sentinel}'",
            "& '" + _quoted(RUNNER) + "' -PrintEnvironment | Out-Null",
            "$cargo = if ($null -eq $env:CARGO_TARGET_DIR) { '<unset>' } else { $env:CARGO_TARGET_DIR }",
            "Write-Output ('AFTER TEMP=' + $env:TEMP + ' TMP=' + $env:TMP + ' CARGO=' + $cargo)",
        )
    )
    result = subprocess.run(
        [str(PWSH), "-NoLogo", "-NoProfile", "-Command", script],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
    )
    assert result.returncode == 0, result.stderr
    assert f"AFTER TEMP={sentinel} TMP={sentinel} CARGO=<unset>" in result.stdout, result.stdout


@requires_pwsh
def test_the_runner_fails_before_python_when_the_build_volume_is_too_full() -> None:
    result = subprocess.run(
        [
            str(PWSH),
            "-NoLogo",
            "-NoProfile",
            "-File",
            str(RUNNER),
            "-MinimumFreeGiB",
            "1000000",
        ],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
    )
    assert result.returncode != 0
    assert "NIOH3_BUILD_VOLUME_LOW_SPACE" in result.stderr, result.stdout + result.stderr


@requires_pwsh
def test_the_runner_executes_a_project_script_in_the_same_environment(tmp_path) -> None:
    python = project_python()
    if python is None:
        pytest.skip("no prepared project Python environment is available")
    probe = tmp_path / "probe.py"
    output = tmp_path / "script-environment.json"
    probe.write_text(
        "import json, os, pathlib, sys\n"
        "pathlib.Path(sys.argv[1]).write_text(json.dumps({\n"
        "    'argument': sys.argv[2],\n"
        "    'TEMP': os.environ.get('TEMP', ''),\n"
        "    'TMP': os.environ.get('TMP', ''),\n"
        "    'CARGO_TARGET_DIR': os.environ.get('CARGO_TARGET_DIR', ''),\n"
        "}), encoding='utf-8')\n",
        encoding="utf-8",
    )
    script = (
        f"& '{_quoted(RUNNER)}'"
        f" -Python '{_quoted(python)}'"
        f" -ScriptPath '{_quoted(probe)}'"
        f" -ScriptArgument @('{_quoted(output)}','sentinel')"
        " -MinimumFreeGiB 0"
    )
    result = subprocess.run(
        [str(PWSH), "-NoLogo", "-NoProfile", "-Command", script],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    recorded = json.loads(output.read_text(encoding="utf-8"))
    assert recorded["argument"] == "sentinel"
    assert recorded["TEMP"] == recorded["TMP"]
    assert same_path(
        recorded["CARGO_TARGET_DIR"], cargo_target.cargo_target_dir(CARGO_CACHE_NAME)
    )
    assert not Path(recorded["TEMP"]).exists(), "the runner removes a successful script temp"


@requires_pwsh
def test_a_real_python_child_sees_the_d_backed_temp(tmp_path) -> None:
    """The point of the runner: Python and its children inherit the D-backed root."""

    python = project_python()
    if python is None:
        pytest.skip("no prepared project Python environment is available")
    if os.environ.get("NIOH3_BUILD_ROOT", "").strip():
        pytest.skip("an explicit build root is in force for this session")
    if os.name != "nt" or os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
        pytest.skip("only a local Windows host is required to prove the D-backed temp")
    if not Path("D:/").exists():
        pytest.skip("this host has no D: delivery volume")

    probe = tmp_path / "child environment probe"
    probe.mkdir()
    probe_test = probe / "test_child_environment.py"
    probe_test.write_text(
        "import json, os, pathlib\n"
        "\n"
        "\n"
        "def test_child_environment():\n"
        "    recorded = {name: os.environ.get(name, '') for name in ('TEMP', 'TMP', 'CARGO_TARGET_DIR')}\n"
        "    pathlib.Path(os.environ['NIOH3_PROBE_OUTPUT']).write_text(json.dumps(recorded), encoding='utf-8')\n",
        encoding="utf-8",
    )
    output = tmp_path / "child environment.json"
    environment = dict(os.environ)
    environment.pop("NIOH3_BUILD_ROOT", None)
    environment["NIOH3_PROBE_OUTPUT"] = str(output)
    # `--rootdir` keeps pytest from walking up to the volume root, which this
    # host cannot collect; the probe is a throwaway module, not a repo suite.
    script = (
        f"& '{_quoted(RUNNER)}'"
        f" -Python '{_quoted(python)}'"
        f" -TestPath '{_quoted(probe_test)}'"
        f" -PytestArgument @('--rootdir','{_quoted(probe)}','-q')"
    )
    result = subprocess.run(
        [str(PWSH), "-NoLogo", "-NoProfile", "-Command", script],
        cwd=str(ROOT),
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=600,
    )
    assert result.returncode == 0, f"{result.stdout}\n{result.stderr}"
    recorded = json.loads(output.read_text(encoding="utf-8"))

    assert recorded["TEMP"] == recorded["TMP"]
    run_temp = Path(recorded["TEMP"])
    assert same_path(recorded["CARGO_TARGET_DIR"], cargo_target.cargo_target_dir(CARGO_CACHE_NAME))
    assert str(run_temp).lower().startswith(str(cargo_target.LOCAL_BUILD_ROOT).lower())
    assert not same_path(run_temp, Path(tempfile.gettempdir()))
    assert not run_temp.is_dir(), "a clean run removes its own temp tree after pytest exits"
