"""Archive only the verified portable manifest, excluding local user state."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('portable', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = args.portable.resolve(strict=True)
    manifest_path = root / 'build-manifest.json'
    manifest = json.loads(manifest_path.read_bytes())
    if manifest['schema'] not in ('nioh3-portable-manifest/v2', 'nioh3-tauri-manifest/v1'):
        raise ValueError('Unsupported portable manifest')
    files, seen = [], set()
    for entry in manifest['files']:
        name = PurePosixPath(entry['path'])
        if name.is_absolute() or any(part in ('..', '.') for part in name.parts) or '\\' in entry['path'] or ':' in entry['path']:
            raise ValueError('Unsafe archive path')
        path = (root / name).resolve(strict=True)
        if not path.is_relative_to(root) or not path.is_file() or entry['path'].lower() in seen:
            raise ValueError('Invalid archive member')
        raw = path.read_bytes()
        if len(raw) != entry['size'] or hashlib.sha256(raw).hexdigest() != entry['sha256']:
            raise ValueError('Portable file differs: ' + entry['path'])
        seen.add(entry['path'].lower())
        files.append((path, entry['path']))
    files.append((manifest_path, 'build-manifest.json'))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # Electron's reproducible resources can have Unix epoch timestamps.
    with zipfile.ZipFile(args.output, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as archive:
        for path, name in files:
            archive.write(path, name)
    with zipfile.ZipFile(args.output) as archive:
        if archive.testzip() is not None or len(archive.namelist()) != len(files):
            raise ValueError('Archive verification failed')
        for entry in manifest['files']:
            with archive.open(entry['path']) as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            if digest != entry['sha256'] or archive.getinfo(entry['path']).file_size != entry['size']:
                raise ValueError('Archived content differs from the portable manifest')
    with args.output.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    args.output.with_suffix('.sha256').write_text(f'{digest}  {args.output.name}\n', encoding='ascii')
    print(json.dumps({'path': str(args.output), 'version': manifest['version'],
                      'files': len(files), 'bytes': args.output.stat().st_size, 'sha256': digest}))


if __name__ == '__main__':
    main()
