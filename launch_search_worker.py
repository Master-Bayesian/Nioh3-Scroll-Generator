"""PyInstaller entry point for the offline-only search role."""
from nioh3_scroll_editor.search_worker import main

if __name__ == '__main__':
    raise SystemExit(main())
