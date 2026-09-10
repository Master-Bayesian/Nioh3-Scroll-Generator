"""Record the actual worker build environment and preserve installed notices."""
from importlib import metadata
import json
from pathlib import Path
import shutil
import sys

target = Path(sys.argv[1]).resolve()
lock = Path(__file__).resolve().parents[1] / 'packaging/requirements-v2.lock.txt'
for line in lock.read_text(encoding='utf-8-sig').splitlines():
    if not line or line.startswith('#'):
        continue
    name, expected = line.split('==')
    if metadata.version(name) != expected:
        raise RuntimeError(f'Build dependency differs from lock: {name}')
target.mkdir(parents=True, exist_ok=True)
notices = target / 'licenses/python'
notices.mkdir(parents=True, exist_ok=True)
packages = []
for distribution in sorted(metadata.distributions(), key=lambda d: d.metadata['Name'].lower()):
    name = distribution.metadata['Name']
    files = []
    # Include notices conservatively, even for build-only packages. The manifest
    # explicitly describes an environment inventory, not an exact linked SBOM.
    for item in distribution.files or []:
        if '.dist-info' not in str(item) or not any(token in item.name.lower() for token in ('license', 'copying', 'notice')):
            continue
        source = Path(distribution.locate_file(item))
        if not source.is_file():
            continue
        destination = notices / name / Path(str(item)).name
        if destination.exists():
            destination = destination.with_name(str(len(files)) + '-' + destination.name)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        files.append(destination.relative_to(target).as_posix())
    packages.append({'name': name, 'version': distribution.version, 'notices': files,
                     'license_expression': distribution.metadata.get('License-Expression')})
python_license = Path(sys.base_prefix) / 'LICENSE.txt'
if not python_license.is_file():
    raise FileNotFoundError('CPython LICENSE.txt is required for the portable artifact')
shutil.copyfile(python_license, notices / 'CPython-LICENSE.txt')
(target / 'python-build-environment.json').write_text(json.dumps({
    'schema': 'nioh3-python-build-environment/v1', 'python': sys.version.split()[0],
    'scope': 'Installed build environment; includes build-only packages, not an exact linked dependency graph.',
    'packages': packages,
}, indent=2) + '\n', encoding='utf-8')
