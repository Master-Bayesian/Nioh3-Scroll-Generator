"""Installer configuration keeps the complete portable product in one download."""
import json
from pathlib import Path
import tempfile
import unittest

from tools.build_tauri_installer import bundle_config, resource_map


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


if __name__ == '__main__':
    unittest.main()
