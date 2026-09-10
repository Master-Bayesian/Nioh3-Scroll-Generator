"""Prepare a single local research insertion with backup and current-state binding.

This script never invokes native code or writes game/save memory. The resulting
Lua file is a separate explicit execution step. Not a public application API.
"""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from research.capture_live_scroll_inventory_readonly import capture
from research.capture_scroll_serial_index import capture as capture_index
from research.dump_effect_catalog_current_locale import ProcessReader
from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool, SCROLL_GROUP_OFFSET


def main():
    directory = ROOT / 'deliverables/frontend-v2/live-add-followup/single-insertion-01'
    directory.mkdir(exist_ok=False)
    prior = ROOT / 'deliverables/frontend-v2/live-acceptance/20260907T225708Z'
    backup_source = Path(json.loads((prior / 'backup-manifest.json').read_text())['source'])
    manifest = {'created_at_utc': datetime.now(timezone.utc).isoformat(), 'files': []}
    for relative in ('SAVEDATA00/SAVEDATA.BIN', 'SAVEDATA00/BACKUP.BIN', 'SYSTEMSAVEDATA00/SAVEDATA.BIN'):
        source = backup_source / relative
        raw = source.read_bytes()
        target = directory / 'backup' / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open('xb') as stream:
            stream.write(raw)
        if source.read_bytes() != raw or target.read_bytes() != raw:
            raise RuntimeError('Backup source changed or copy differs')
        manifest['files'].append({'relative_path': relative, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()})
    (directory / 'backup-manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    decrypted = directory / 'backup-decrypted.bin'
    SaveCrypto(default_crypto_tool(ROOT)).decrypt(directory / 'backup/SAVEDATA00/SAVEDATA.BIN', decrypted)
    saved = decrypted.read_bytes()
    inventory = capture()
    index = capture_index()
    index_by_serial = {e['serial']: e['slot'] for e in index['entries']}
    saved_records = {}
    for slot in range(400):
        record = saved[SCROLL_GROUP_OFFSET+slot*0xE8:SCROLL_GROUP_OFFSET+(slot+1)*0xE8]
        if struct.unpack_from('<H', record)[0]:
            saved_records[str(struct.unpack_from('<Q', record, 0x28)[0])] = record
    if set(saved_records) != {e['serial'] for e in inventory['entries']}:
        raise RuntimeError('Backup and runtime occupied serials differ; save before proceeding')
    for entry in inventory['entries']:
        record = bytes.fromhex(entry['record_hex'])
        baseline = saved_records[entry['serial']]
        if record[:0x24] != baseline[:0x24] or record[0x28:0xE4] != baseline[0x28:0xE4]:
            raise RuntimeError('Backup and runtime defined scroll fields differ')
        if index_by_serial.get(entry['serial']) != entry['slot_index']:
            raise RuntimeError('Existing native serial index disagrees with occupied inventory')
    preview = json.loads((directory.parent / 'assembly-preview-plan.json').read_text())
    descriptor = bytearray.fromhex(preview['descriptor_hex'])
    descriptor[0x21] = 0
    with ProcessReader() as reader:
        if reader.pid != inventory['pid'] or index['pid'] != reader.pid:
            raise RuntimeError('Process changed')
        manager = reader.u64(reader.module_base + 0x474D4E0)
        data = reader.u64(manager)
        container = reader.read(data + 0x224A60, 400*0xE8)
        serial = reader.u64(data+8)
        if str(serial) != inventory['serial_counter'] or str(serial) in index_by_serial:
            raise RuntimeError('Serial changed or already indexed')
        free = next((s for s in range(400) if container[s*0xE8:s*0xE8+2] == b'\0\0'), None)
        if free is None:
            raise RuntimeError('No free slot')
        scheduler = reader.u64(reader.module_base + 0x47412F8)
        if reader.read(scheduler+0x1408,4) != bytes(4) or reader.read(scheduler+0x1629,1) != b'\1':
            raise RuntimeError('Observed mission task phase is not idle')
        reference = ROOT / 'audit/runtime_sections/v2.0.1.0_20260902_title/Nioh3_v2.0.1.0.text.bin'
        original = reference.read_bytes()[0x54D294-0x1000:0x54E0AB-0x1000]
        if reader.read(reader.module_base+0x54D294,len(original)) != original:
            raise RuntimeError('Insertion function differs from reviewed code')
        plan = {'pid': reader.pid, 'manager': manager, 'data': data, 'serial': serial,
                'slot': free, 'operation_id': str(uuid.uuid4()), 'scheduler_owner': scheduler,
                'function_address': reader.module_base+0x54D294, 'container_hex': container.hex(),
                'insertion_code_hex': original.hex()}
    assembly = {key: preview[key] for key in ('expected_record_hex', 'builder_code_hex')}
    assembly['descriptor_hex'] = descriptor.hex()
    for name, value in [('inventory-before.json',inventory),('index-before.json',index),
                        ('insertion-plan.json',plan),('assembly-plan.json',assembly)]:
        (directory/name).write_text(json.dumps(value,indent=2),encoding='utf-8')
    def table(value):
        return '{\n' + ''.join(k+'='+ (json.dumps(v) if isinstance(v,str) else str(v))+',\n' for k,v in value.items()) + '}'
    lua = ('nioh3DispatchProbeOptions={assembly_preview='+table(assembly)+',single_insertion='+table(plan)+'}\n'
           'dofile("F:/Nioh3_ScrollEditor/research/probe_pickup_dispatch_noop_ce.lua")\n')
    (directory/'arm-once.lua').write_text(lua,encoding='utf-8')
    print(json.dumps({'directory':str(directory),'operation_id':plan['operation_id'],
                      'backup_matches_runtime_scrolls':len(inventory['entries']),
                      'expected_serial':serial,'planned_slot':free,'native_invoked':False}))


if __name__ == '__main__':
    main()
