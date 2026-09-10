# -*- mode: python ; coding: utf-8 -*-
from pathlib import Path

root = Path(SPECPATH).parent
crypto = root / 'bin/Nioh_Savefile_decrypt.exe'
if not crypto.is_file():
    crypto = root / '.tools/nioh3-save-crypt-source/x64/Release/Nioh_Savefile_decrypt.exe'
a = Analysis(
    [str(root / 'launch_protected_worker.py')], pathex=[str(root)],
    binaries=[(str(root / 'bin/nioh3_seed_accelerator.dll'), 'bin'),
              (str(root / 'bin/nioh3_effect_preimage_accelerator.dll'), 'bin'),
              (str(crypto), 'bin')],
    datas=[(str(root / 'nioh3_scroll_editor/data'), 'nioh3_scroll_editor/data'),
           *[(str(path), 'packages/contracts') for path in (root / 'packages/contracts').glob('*.schema.json')],
           (str(root / 'bin/nioh3_seed_accelerator.build.json'), 'bin')],
    hiddenimports=[], excludes=['tkinter'], noarchive=False, optimize=0,
)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, a.binaries, a.datas, [], name='nioh3-protected-worker',
          debug=False, strip=False, upx=False, console=True)
