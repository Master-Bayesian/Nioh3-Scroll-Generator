"""Authorize signing only the exact hosted candidate already accepted locally."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1] if len(sys.argv) > 1 else 'deliverables/release/portable')
evidence = json.loads(Path('docs/knowledge/evidence/tauri-v071-acceptance.json').read_text(encoding='utf-8-sig'))
manifest_bytes = (root / 'build-manifest.json').read_bytes()
manifest = json.loads(manifest_bytes)
assert evidence['schema'] == 'nioh3-tauri-release-acceptance/v1'
assert evidence['candidateRun'] == os.environ['CANDIDATE_RUN']
assert evidence['version'] == manifest['version'] == '0.7.1'
assert manifest['git'] == {'commit': evidence['sourceCommit'], 'dirty': False}
assert hashlib.sha256(manifest_bytes).hexdigest() == evidence['manifestSha256']
assert hashlib.sha256((root / 'Nioh3Studio.exe').read_bytes()).hexdigest() == evidence['exeSha256']
assert sum(f['size'] for f in manifest['files']) < 60 * 1024 * 1024
for key in ['webview2', 'pythonHandshake', 'favorites', 'isolatedInventory', 'privateMethodRefused']:
    assert evidence['ui'][key] is True
for key in ['replacement', 'realApplicationStarted', 'workerHandshake', 'previousRemoved', 'cacheRemoved']:
    assert evidence['update'][key] is True
assert evidence['ui']['gameWrites'] == evidence['update']['gameWrites'] == 0
# Infrastructure and evidence may advance; the accepted product must not.
subprocess.run(['git', 'diff', '--exit-code', evidence['sourceCommit'], 'HEAD', '--',
                'apps/workshop', 'apps/tauri/src-tauri', 'apps/tauri/bridge.ts', 'apps/tauri/entry.ts',
                'apps/tauri/build.mjs', 'nioh3_scroll_editor', 'packages', 'assets', 'third_party',
                'package.json', 'package-lock.json', 'packaging', 'tools/package_tauri.py',
                'tools/build_tauri.ps1'], check=True)
print(json.dumps({'accepted': True, 'commit': evidence['sourceCommit'], 'manifestSha256': evidence['manifestSha256']}))
