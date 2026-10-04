import { test } from 'node:test';
import assert from 'node:assert/strict';
// @ts-expect-error The build helper is plain JavaScript.
import { darkColor, withDarkTheme } from '../../workshop/dark-theme.mjs';

const lightness = (hex: string) => {
  const [r, g, b] = [1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

test('dark colors flip lightness and keep the order of every pair', () => {
  const white = darkColor('ffffffff');
  const black = darkColor('000000ff');
  assert.ok(lightness(white) < 60, white);
  assert.ok(lightness(black) > 200, black);
  const ramp = ['f8fafc', 'c5d1e0', '58717b', '214d63', '18272d'].map(hex => lightness(darkColor(hex + 'ff')));
  assert.deepEqual([...ramp].sort((a, b) => a - b), ramp, 'lighter in light mode is darker in dark mode');
  assert.equal(darkColor('ffffff99').slice(-2), '99', 'alpha is kept');
});

test('declaration colors become tokens; selectors and ids are untouched', () => {
  const css = withDarkTheme('#add:hover{color:#fff;border:1px solid #c5d1e0cc}.x{--fill:#abc}');
  assert.match(css, /#add:hover\{color:var\(--k0\);border:1px solid var\(--k1\)\}/);
  assert.match(css, /--fill:var\(--k2\)/);
  assert.match(css, /^:root\{--k0:#ffffff;--k1:#c5d1e0cc;--k2:#aabbcc\}/);
  assert.match(css, /:root\[data-theme=dark\]\{--k0:#[0-9a-f]{6};/);
  assert.match(css, /@media\(prefers-color-scheme:dark\)\{:root:not\(\[data-theme=light\]\)\{/);
});

test('game-styled panels keep their colors in both themes', () => {
  const css = withDarkTheme('.scroll{background:#222d29}.scroll-audit{color:#63798d}.scroll .x{color:#dce7e5}.result-pane{background:#fff}@media(min-width:9px){.effect-line{color:#a4cfdb}.y{color:#fff}}');
  assert.match(css, /\.scroll\{background:#222d29\}/);
  assert.match(css, /\.scroll \.x\{color:#dce7e5\}/);
  assert.match(css, /\.result-pane\{background:var\(--k\w+\)\}/);
  assert.match(css, /@media\(min-width:9px\)\{\.effect-line\{color:#a4cfdb\}\.y\{color:var\(--k\w+\)\}\}/);
  assert.match(css, /\.scroll-audit\{color:var\(--k\w+\)\}/);
});
