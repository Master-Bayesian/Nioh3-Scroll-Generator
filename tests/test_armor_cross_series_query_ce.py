"""Native-query ownership, authorization, bounds, and incomplete-call checks."""
from copy import deepcopy
import json
from pathlib import Path
import sys

from lupa import LuaRuntime
import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from armor_remodel_batch_v202 import ArmorModel, MODES, EXE_SHA


def mock_config():
    model = ArmorModel()
    raw = (ROOT / 'nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/tables/item.bin').read_bytes()
    vectors = []
    for item_id in (0xC18E, 0x35BC, 0xC288):
        for mode in MODES:
            value = model.calculate(item_id, stage=3, mode=mode)
            original, selected = model.items[item_id], model.items[value['selected_row_id']]
            vectors.append({'item_id': item_id, 'mode': mode, 'row_index': original.index,
                            'selected_row_index': selected.index,
                            'row_hex': raw[8 + original.index * 416:8 + (original.index + 1) * 416].hex(),
                            'selected_row_hex': raw[8 + selected.index * 416:8 + (selected.index + 1) * 416].hex(),
                            'expected_weight': value['weight_raw'], 'expected_toughness': value['toughness'],
                            'expected_requirements': value['requirements']})
    sites = (0x818018, 0x817FD4, 0x2F9E30, 0x8180AC, 0x2F9E88, 0x2F9EEC,
             0x2FB078, 0x2F9F34, 0x1111850, 0xF4DBC, 0xC624A8)
    return {'pid': 777, 'module_base': 0x140000000, 'creation_filetime': '1234567890',
            'executable_sha256': EXE_SHA, 'run_id': 'mock-cross-series', 'stage': 3,
            'manager': 0x20000000, 'item_container': 0x20001000, 'item_raw': 0x30000000,
            'curve_container': 0x20002000, 'curve_raw': 0x40000000,
            'stage_container': 0x20003000, 'stage_object': 0x20004000,
            'item_map': 0x20005000, 'item_map_begin': 0x20006000, 'item_map_end': 0x20007000,
            'signatures': [{'rva': site, 'size': 4, 'hex': '909090C3'} for site in sites],
            'vectors': vectors}


def setup_query(failure=None):
    config = mock_config()
    config['authorized_native_queries'] = True
    config['expected_age_ms'] = 20000
    memory = {}
    calls = []
    allocations = []
    releases = []
    scratch = 0x100000000

    def store(address, values):
        for i, value in enumerate(values):
            memory[address + i] = value

    def word(address, value, width=8):
        store(address, int(value).to_bytes(width, 'little'))

    base = config['module_base']
    word(base + 0x45B9E30, config['manager'])
    word(config['manager'] + 0x68, config['item_container'])
    word(config['item_container'], config['item_raw'])
    word(config['item_raw'] + 4, 3362, 4)
    word(config['item_container'] + 0x20, config['item_map'])
    word(config['item_map'] + 8, config['item_map_begin'])
    word(config['item_map'] + 0x10, config['item_map_end'])
    word(config['manager'] + 0x70, config['curve_container'])
    word(config['curve_container'], config['curve_raw'])
    word(config['curve_raw'] + 4, 502, 4)
    word(base + 0x47514D0, config['stage_container'])
    word(config['stage_container'] + 8, config['stage_object'])
    word(config['stage_object'] + 0x45, config['stage'], 1)
    for site in config['signatures']:
        store(base + site['rva'], bytes.fromhex(site['hex']))
    for vector in config['vectors']:
        store(config['item_raw'] + 8 + vector['row_index'] * 416, bytes.fromhex(vector['row_hex']))
        store(config['item_raw'] + 8 + vector['selected_row_index'] * 416, bytes.fromhex(vector['selected_row_hex']))
    lua = LuaRuntime(unpack_returned_tuples=True)
    api = lua.execute((ROOT / 'research/armor_cross_series_query_ce.lua').read_text(encoding='utf-8'))
    env = lua.globals()
    env.getOpenedProcessID = lambda: config['pid']
    env.getAddressSafe = lambda _: base
    env.getOpenedProcessHandle = lambda: 123
    env.getProcessAge = lambda: 20000
    env.getTickCount = lambda: 1000
    env.debug_isDebugging = lambda: False
    env.readBytes = lambda address, count, _: lua.table_from([memory[address + i] for i in range(count)])

    def allocate(size, address, protection):
        assert (size, address, protection) == (272, None, 4)
        allocations.append(scratch)
        return scratch

    def write(address, values):
        assert address == scratch and len(values) == 272
        store(address, [values[i] for i in range(1, 273)])
        return True

    def release(address):
        assert address == scratch and address not in releases
        releases.append(address)
        return True

    def execute(convention, timeout, address, *params):
        assert (convention, timeout) == (0, 1000)
        assert params[0]['type'] == params[1]['type'] == 0
        assert params[1]['value'] == scratch + 16
        raw = bytes(memory[scratch + 16 + i] for i in range(240))
        assert int.from_bytes(raw[6:8], 'little') == 180
        assert int.from_bytes(raw[10:12], 'little') == 20
        assert raw[48] == 4 and int.from_bytes(raw[24:28], 'little') == 0
        item_id = int.from_bytes(raw[:2], 'little')
        mode = f'{raw[49]}/{raw[50]}'
        vector = next(v for v in config['vectors'] if v['item_id'] == item_id and v['mode'] == mode)
        assert params[0]['value'] == config['item_raw'] + 8 + vector['row_index'] * 416
        rva = address - base
        assert rva in (0x818018, 0x817FD4, 0x2F9E30)
        calls.append((item_id, mode, rva))
        if failure == 'timeout':
            return None
        if failure == 'scratch':
            memory[scratch] = 0
        if failure == 'attachment':
            env.getOpenedProcessHandle = lambda: 999
        if rva == 0x2F9E30:
            assert params[2]['type'] == 0 and params[2]['value'] in range(7)
            value = vector['expected_requirements'][params[2]['value']]
        else:
            value = vector['expected_weight' if rva == 0x818018 else 'expected_toughness']
        return value + (1 if failure == 'mismatch' else 0)

    env.allocateMemory, env.writeBytes, env.deAlloc, env.executeCodeEx = allocate, write, release, execute
    return lua, api, config, calls, allocations, releases


def test_explicit_authorization_is_required_before_any_allocation():
    lua, api, config, calls, allocations, releases = setup_query()
    config['authorized_native_queries'] = False
    with pytest.raises(Exception, match='Explicit authorization'):
        api.run(lua.table_from(config, recursive=True))
    assert calls == allocations == releases == []


def test_complete_bounded_pass_releases_owned_scratch():
    lua, api, config, calls, allocations, releases = setup_query()
    result = api.run(lua.table_from(config, recursive=True))
    assert result['success'] and result['released'] and not result['in_flight']
    assert result['native_calls'] == len(result['events']) == len(calls) == 162
    assert len({(item, mode) for item, mode, _ in calls}) == 18
    assert len(allocations) == len(releases) == 1


@pytest.mark.parametrize('failure', ['mismatch', 'scratch'])
def test_completed_bad_query_stops_and_cleans(failure):
    lua, api, config, calls, allocations, releases = setup_query(failure)
    result = api.run(lua.table_from(config, recursive=True))
    assert not result['success'] and result['error'] and result['released']
    assert len(calls) == 1 and len(releases) == 1


@pytest.mark.parametrize('failure', ['timeout', 'attachment'])
def test_unconfirmed_call_or_changed_attachment_retains_allocation(failure):
    lua, api, config, calls, allocations, releases = setup_query(failure)
    result = api.run(lua.table_from(config, recursive=True))
    assert not result['success'] and not result['released'] and result['cleanup_error']
    assert len(calls) == 1 and len(allocations) == 1 and releases == []
    with pytest.raises(Exception, match='Previous native query allocation'):
        api.run(lua.table_from(config, recursive=True))
    assert len(calls) == len(allocations) == 1


def test_wrong_signature_and_duplicate_vector_fail_before_allocation():
    for corruption in ('signature', 'duplicate'):
        lua, api, config, calls, allocations, releases = setup_query()
        if corruption == 'signature':
            config['signatures'][0]['hex'] = '00' + config['signatures'][0]['hex'][2:]
        else:
            config['vectors'][1] = deepcopy(config['vectors'][0])
        with pytest.raises(Exception):
            api.run(lua.table_from(config, recursive=True))
        assert calls == allocations == releases == []
