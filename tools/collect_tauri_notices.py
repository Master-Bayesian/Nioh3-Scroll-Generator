"""Collect missing crate notices from their exact published source commits."""
import concurrent.futures
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parents[1]

def collect(package):
    source = Path(package['manifest_path']).parent
    if any(p.is_file() and p.name.lower().startswith(('license', 'copying', 'notice')) for p in source.iterdir()):
        return
    vcs = json.loads((source / '.cargo_vcs_info.json').read_text())
    repo = package['repository'].rstrip('/').removesuffix('.git')
    if not repo.startswith('https://github.com/'):
        raise ValueError('Review repository: ' + repo)
    base = repo.replace('https://github.com/', 'https://raw.githubusercontent.com/') + '/' + vcs['git']['sha1']
    directory = Path(vcs.get('path_in_vcs', ''))
    notices = []
    for parent in [directory, *directory.parents]:
        for name in ('LICENSE', 'LICENSE.txt', 'LICENSE-MIT', 'LICENSE-APACHE', 'COPYING', 'LICENSE.md', 'LICENCE', 'LICENCE.md', 'LICENSE-MIT.txt', 'LICENSE-APACHE.txt', 'LICENSE_MIT', 'LICENSE_APACHE-2.0'):
            relative = (parent / name).as_posix()
            url = base + '/' + relative
            try:
                with urllib.request.urlopen(url, timeout=20) as response:
                    raw = response.read(500000)
            except urllib.error.HTTPError as error:
                if error.code == 404:
                    continue
                raise
            destination = ROOT / 'third_party/rust-notices' / (package['name'] + '-' + package['version'])
            destination.mkdir(parents=True, exist_ok=True)
            (destination / name).write_bytes(raw)
            notices.append({'file': name, 'source': url, 'sha256': hashlib.sha256(raw).hexdigest()})
        if notices:
            (destination / 'provenance.json').write_text(json.dumps(notices, indent=2) + '\n')
            print(package['name'], 'OK', flush=True)
            return
    if package['license'] == 'MPL-2.0':
        url = 'https://www.mozilla.org/media/MPL/2.0/index.txt'
        raw = urllib.request.urlopen(url, timeout=20).read()
        destination = ROOT / 'third_party/rust-notices' / (package['name'] + '-' + package['version'])
        destination.mkdir(parents=True, exist_ok=True)
        (destination / 'LICENSE-MPL-2.0.txt').write_bytes(raw)
        (destination / 'SOURCE.txt').write_text('Unmodified source, including original copyright notices: ' + repo + '/tree/' + vcs['git']['sha1'] + '/' + vcs.get('path_in_vcs', '') + '\n')
        (destination / 'provenance.json').write_text(json.dumps([{'file': 'LICENSE-MPL-2.0.txt', 'source': url, 'sha256': hashlib.sha256(raw).hexdigest()}], indent=2) + '\n')
        return
    raise ValueError('Missing notice: ' + package['name'])

if __name__ == '__main__':
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(ROOT / 'apps/tauri/src-tauri/Cargo.toml'), '--format-version', '1', '--locked', '--filter-platform', 'x86_64-pc-windows-msvc']))
    used = {n['id'] for n in metadata['resolve']['nodes']}
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        list(executor.map(collect, [p for p in metadata['packages'] if p['source'] and p['id'] in used]))
