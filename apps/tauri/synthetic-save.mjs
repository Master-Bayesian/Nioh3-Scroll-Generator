import {execFileSync} from 'node:child_process';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

/**
 * Run the Rust synthetic-save fixture (`crates/nioh3-save/examples/synthetic_save.rs`).
 *
 * `save <new dir>` and `restore <dir>` build an isolated encrypted save;
 * `decrypt <container> <output>` writes its clear bytes. Each prints one JSON line.
 */
export function syntheticSave(...args) {
  const output = execFileSync(
    'cargo',
    [
      'run', '--release', '--quiet', '--locked',
      '--manifest-path', join(ROOT, 'crates/nioh3-save/Cargo.toml'),
      '--example', 'synthetic_save', '--', ...args,
    ],
    {cwd: ROOT, windowsHide: true, stdio: ['ignore', 'pipe', 'inherit'], timeout: 600000},
  );
  return JSON.parse(output.toString('utf8'));
}
