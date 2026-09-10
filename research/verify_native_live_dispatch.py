"""Live no-insertion acceptance for the fixed Windows executor."""
import json
from pathlib import Path
import sys
import time
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.live_add_native_transport import NativeLiveAddTransport
from nioh3_scroll_editor.live_inventory import capture_inventory
from nioh3_scroll_editor.live_add_profile import PC_V201
from nioh3_scroll_editor.dispatch_evidence import verify_dispatch

root = Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=False)
before = capture_inventory()
(root / 'before.json').write_text(json.dumps(before, indent=2), encoding='utf-8')
transport = NativeLiveAddTransport(root / 'executor')
operation = str(uuid4())
transport.call('noop', operation_id=operation, pid=before['pid'], profile_id=PC_V201.profile_id)
deadline = time.monotonic() + 20
while True:
    result = transport.call('status', operation_id=operation)
    if result['released']:
        break
    if time.monotonic() > deadline:
        print(json.dumps({'pending': result}), flush=True)
        deadline = time.monotonic() + 20
    time.sleep(0.05)
result['breakpoints'] = [] if result['breakpoint_count'] == 0 else ['unconfirmed']
verify_dispatch(result)
after = capture_inventory()
assert before['container_sha256'] == after['container_sha256']
assert before['serial_counter'] == after['serial_counter']
(root / 'after.json').write_text(json.dumps(after, indent=2), encoding='utf-8')
(root / 'verification.json').write_text(json.dumps({'execution': result, 'inventory_unchanged': True}, indent=2), encoding='utf-8')
print(json.dumps({'result': 'NATIVE_LIVE_NOOP_OK', 'thread': result['thread_id'], 'inventory_unchanged': True}), flush=True)
