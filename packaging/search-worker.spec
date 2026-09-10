# -*- mode: python ; coding: utf-8 -*-
from pathlib import Path

root = Path(SPECPATH).parent
a = Analysis(
    [str(root / 'launch_search_worker.py')], pathex=[str(root)],
    binaries=[(str(root / 'bin/nioh3_seed_accelerator.dll'), 'bin'),
              (str(root / 'bin/nioh3_effect_preimage_accelerator.dll'), 'bin')],
    datas=[(str(root / 'nioh3_scroll_editor/data'), 'nioh3_scroll_editor/data'),
           (str(root / 'packages/contracts/request.schema.json'), 'packages/contracts'),
           (str(root / 'packages/contracts/response.schema.json'), 'packages/contracts'),
           (str(root / 'bin/nioh3_seed_accelerator.build.json'), 'bin')],
    hiddenimports=[], excludes=['tkinter'], noarchive=False, optimize=0,
)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, a.binaries, a.datas, [], name='nioh3-search-worker',
          debug=False, strip=False, upx=False, console=True)
