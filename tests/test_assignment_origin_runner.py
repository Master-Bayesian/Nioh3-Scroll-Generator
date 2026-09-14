from pathlib import Path
import sys,json,importlib.util
import pytest
ROOT=Path(__file__).resolve().parents[1];CAP=ROOT/'research'/'possessed_enemy_capture'
sys.path.insert(0,str(CAP));sys.path.insert(0,str(Path(__file__).parent))
import run_possessed_enemy_observer as runner
from lua54_test_runtime import Lua54

def test_runner_phase_maps_actual_file():
    assert runner.PHASES['assignment-origin']=='assignment_origin_ce.lua'
    assert (CAP/runner.PHASES['assignment-origin']).is_file()

def test_lua_identity_encoder_handles_unicode_and_windows_path():
    val={'image_path':'C:\\游戏\\Nioh3.exe','creation_filetime':'123456789012345678','process_id':123,
         'utf8':'中文','control':'\x00\x01','true':True,'false':False}
    L=Lua54();assert L.run('return '+runner.lua_literal(val))==val

def test_existing_callable_bridge_export_retained():assert callable(runner.CheatEngineBridge)

def test_windows_birth_query_fails_explicitly_off_windows():
    if sys.platform!='win32':
        with pytest.raises(RuntimeError,match='Windows'):runner.process_birth_filetime(123)

def test_no_overwrite_evidence_or_pending_receipt(tmp_path):
    p=tmp_path/'trace.json';p.with_suffix('.cleanup-pending.json').write_text('{}')
    with pytest.raises(FileExistsError):runner.require_unused_output(p)

def test_ambiguous_bridge_not_silently_redirected():
    with pytest.raises(RuntimeError):runner.select_session([{'session_id':'a'},{'session_id':'b'}],None)
    assert runner.select_session([{'session_id':'a'},{'session_id':'b'}],'b')=='b'

def test_signature_parser_covers_all_new_sites():
    spec=importlib.util.spec_from_file_location('collector_validator',ROOT/'tools'/'validate_possessed_enemy_collectors.py')
    m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
    expected=json.loads((CAP/'assignment_origin_v201_locators.json').read_text())['sites']
    sites=m.parse_expected((CAP/'assignment_origin_ce.lua').read_text())
    assert len(sites)==4
    assert {n:(r,b.hex().upper())for n,r,b in sites}=={n:(int(x['rva'],16),x['bytes'])for n,x in expected.items()}

def test_new_observer_has_no_target_write_or_native_call_api():
    source=(CAP/'assignment_origin_ce.lua').read_text()
    for name in ('writeBytes(','writeInteger(','writeQword(','executeCode(','executeCodeEx(','autoAssemble('):
        assert name not in source


def test_cleanup_identity_uses_lua_bytes_not_json_unicode():
    name='附身观察\\路径]]'
    L=Lua54()
    L.run("nioh3PossessedCapture={run_id="+runner.lua_literal(name)+",active=true,cleanup_pending=false,owned_breakpoints={}};"
          "function nioh3PossessedCapture.stop(reason) nioh3PossessedCapture.active=false; "
          "nioh3PossessedCapture.stop_reason=reason end; function debug_getBreakpointList() return {} end; "
          "function debug_isBroken() return false end; return true")
    result=L.run(runner.cleanup_script(name,'手动停止'))
    assert result['probe']['run_id']==name
    assert result['probe']['active'] is False
    assert result['probe']['stop_reason']=='手动停止'

def test_runner_does_not_report_incomplete_origin_timeout_as_success():
    source=(CAP/'run_possessed_enemy_observer.py').read_text()
    assert 'raise TimeoutError(f"{args.phase} capture did not finish before the runner deadline")' in source
    assert 'NIOH3_POSSESSED_RUN_ID={lua_literal(args.run_id)}' in source
    assert 'loadfile({lua_literal(lua_path(phase_path))})' in source


def _fresh_lua(prior='nil', debugging='true', broken='false', inventory='{}'):
    L = Lua54()
    prelude = (f"nioh3PossessedCapture={prior}; "
        f"function debug_isDebugging() return {debugging} end; "
        f"function debug_isBroken() return {broken} end; "
        f"function debug_getBreakpointList() return {inventory} end;")
    return L, prelude


def test_fresh_phase_handshake_clears_only_owned_namespace_and_proves_empty_state():
    L, prelude = _fresh_lua()
    result = L.run(prelude + runner.fresh_phase_script())
    assert result == {'initialized': True, 'debugger_active': True,
                      'debugger_broken': False,
                      'debugger_broken_source': 'debug_isBroken',
                      'breakpoints': {}}
    assert L.run('return nioh3PossessedCapture') is None
    assert L.run('return NIOH3_POSSESSED_RUN_ID') is None


@pytest.mark.parametrize('case', [
    (_fresh_lua(prior="{active=true,cleanup_pending=false,owned_breakpoints={}}"), 'active'),
    (_fresh_lua(prior="{active=false,cleanup_pending=true,owned_breakpoints={}}"), 'cleanup state'),
    (_fresh_lua(prior="{active=false,cleanup_pending=false,owned_breakpoints={'0x1'}}"), 'ownership'),
    (_fresh_lua(prior="{}"), 'active state'),
    (_fresh_lua(inventory="{1234}"), 'Foreign'),
    (_fresh_lua(broken='true'), 'stopped'),
    (_fresh_lua(inventory='nil'), 'unknown'),
])
def test_fresh_phase_handshake_fails_closed(case):
    (L, code), pattern = case
    with pytest.raises(RuntimeError, match=pattern):
        L.run(code + runner.fresh_phase_script())


def test_arm_verification_requires_exact_global_inventory_and_identity():
    L = Lua54()
    code = ("nioh3PossessedCapture={active=true,run_id='r',schema='s',"
            "owned_breakpoints={'0x100','0x200'}}; "
            "function debug_getBreakpointList() return {0x100,0x200} end; "
            "function debug_isBroken() return false end; " + runner.arm_verification_script())
    result = L.run(code)
    assert result['active'] is True and result['owned_breakpoints'] == ['0x100', '0x200']
    assert runner.arm_verified({'result': result}, 'r')
    assert not runner.arm_verified({'result': dict(result, run_id='other')}, 'r')
    assert runner.arm_verified({'result': result}, 'r', expected_breakpoint_count=2)
    assert not runner.arm_verified({'result': result}, 'r', expected_breakpoint_count=4)


def test_arm_verification_rejects_mismatch_and_unknown_inventory():
    for inventory in ('{0x100}', 'nil'):
        L = Lua54()
        code = ("nioh3PossessedCapture={active=true,run_id='r',owned_breakpoints={'0x100','0x200'}}; "
                f"function debug_getBreakpointList() return {inventory} end; "
                "function debug_isBroken() return false end; " + runner.arm_verification_script())
        with pytest.raises(RuntimeError):
            L.run(code)


def test_arm_verification_rechecks_live_pid_module_birth_and_expected_count():
    L = Lua54()
    prelude = (
        "actualPid=777;actualBase=0x140000000;"
        "nioh3PossessedCapture={active=true,run_id='r',schema='s',pid=777,module_base='0x140000000',"
        "owned_breakpoints={'0x100','0x200','0x300','0x400'}};"
        "NIOH3_POSSESSED_TARGET_IDENTITY={creation_filetime='123'};"
        "function getOpenedProcessID() return actualPid end;"
        "function getAddressSafe(_) return actualBase end;"
        "function debug_getBreakpointList() return {0x100,0x200,0x300,0x400} end;"
        "function debug_isBroken() return false end;"
    )
    script = runner.arm_verification_script(
        expected_pid=777,
        expected_module_base="0x140000000",
        expected_creation_filetime="123",
        expected_breakpoint_count=4,
    )
    result = L.run(prelude + script)
    assert runner.arm_verified({"result": result}, "r", expected_breakpoint_count=4)
    for mutation in ("actualPid=778;", "actualBase=0x150000000;"):
        other = Lua54()
        with pytest.raises(RuntimeError):
            other.run(prelude + mutation + script)
