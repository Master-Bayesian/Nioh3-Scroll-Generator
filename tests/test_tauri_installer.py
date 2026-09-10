"""Installer configuration keeps the complete portable product in one download."""
import json
from pathlib import Path
import tempfile
import unittest

from tools.build_tauri_installer import (
    NSIS_BUNDLE_TOKEN,
    UNKNOWN_BUNDLE_TOKEN,
    bundle_config,
    patch_nsis_binary,
    refresh_main_manifest,
    resource_map,
)


class TauriInstallerTests(unittest.TestCase):
    def test_resources_preserve_product_paths_and_exclude_duplicate_main_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Nioh3Studio.exe').write_bytes(b'MZ')
            (root / 'worker').mkdir()
            (root / 'worker/nioh3-search-worker.exe').write_bytes(b'MZ')
            (root / 'build-manifest.json').write_text('{}', encoding='utf-8')
            resources = resource_map(root)
            self.assertNotIn(str((root / 'Nioh3Studio.exe').resolve()), resources)
            self.assertEqual(resources[str((root / 'worker/nioh3-search-worker.exe').resolve())],
                             'worker/nioh3-search-worker.exe')
            self.assertEqual(resources[str((root / 'build-manifest.json').resolve())],
                             'build-manifest.json')
            config = bundle_config(root)
            self.assertEqual(config['mainBinaryName'], 'Nioh3Studio')
            self.assertEqual(config['bundle']['targets'], ['nsis'])
            self.assertEqual(config['bundle']['windows']['nsis']['installMode'], 'currentUser')
            json.dumps(config)

    def test_nsis_marker_and_portable_manifest_are_updated_together(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / 'Nioh3Studio.exe'
            binary.write_bytes(b'MZ-prefix-' + UNKNOWN_BUNDLE_TOKEN + b'-suffix')
            manifest = {
                'files': [{'path': 'Nioh3Studio.exe', 'size': 0, 'sha256': ''}],
            }
            self.assertTrue(patch_nsis_binary(binary))
            self.assertFalse(patch_nsis_binary(binary))
            refresh_main_manifest(root, manifest)
            written = json.loads((root / 'build-manifest.json').read_text(encoding='utf-8'))
            self.assertIn(NSIS_BUNDLE_TOKEN, binary.read_bytes())
            self.assertEqual(written['files'][0]['size'], binary.stat().st_size)
            self.assertEqual(len(written['files'][0]['sha256']), 64)


if __name__ == '__main__':
    unittest.main()
