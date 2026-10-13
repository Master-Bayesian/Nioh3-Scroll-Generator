// Dark theme (#14), derived at build time so style.css keeps plain colors.
//
// Every hex color in a declaration value becomes `var(--kN)`. The light values
// are the originals; the dark values flip each color's OKLab lightness into a
// dark range and keep its hue and (gamut-clamped) chroma, so every contrast the
// light theme draws between two colors survives in the dark one.

const HEX = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b/g;
// A declaration value: after a colon, up to the next `;` or `}`. Selectors end
// at `{`, so pseudo-classes never match.
const VALUE = /:([^;{}]+)(?=;|$)/g;
// One rule: a selector and a declaration block without nested braces, so the
// rules inside an @media block match one by one and its prelude stays as is.
const RULE = /([^{}]+)\{([^{}]*)\}/g;
// The scroll card is drawn like the game's own dark scroll, so it is dark in
// the light theme already; flipping it (and its lines and enemy markers)
// would turn it light in the dark one.
const GAME_STYLED = /\.(?:scroll(?![\w-])|scroll-(?:hero|rule|terrain)\b|effect-line|enemy-lines|enemy-occurrence|enemy-state-marker|rarity(?:-|\b))/;

function expand(hex) {
  let digits = hex.slice(1).toLowerCase();
  if (digits.length <= 4) digits = [...digits].map(digit => digit + digit).join("");
  if (digits.length === 6) digits += "ff";
  return digits;
}

const toLinear = channel => (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
const toGamma = channel => (channel <= 0.0031308 ? 12.92 * channel : 1.055 * channel ** (1 / 2.4) - 0.055);

function rgbToOklab([red, green, blue]) {
  const [r, g, b] = [red, green, blue].map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function oklabToRgb([lightness, a, b]) {
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ].map(toGamma);
}

const inGamut = rgb => rgb.every(channel => channel >= -1e-4 && channel <= 1 + 1e-4);

/** The dark counterpart of one 8-digit hex color. */
export function darkColor(digits) {
  const rgb = [0, 2, 4].map(offset => parseInt(digits.slice(offset, offset + 2), 16) / 255);
  const alpha = digits.slice(6);
  const [lightness, a, b] = rgbToOklab(rgb);
  const target = 0.17 + (1 - lightness) * 0.76;
  // Pale tints turned dark read as mud at full chroma; soften them a little.
  let scale = lightness > 0.85 ? 0.8 : 1;
  let out = oklabToRgb([target, a * scale, b * scale]);
  while (!inGamut(out) && scale > 0) {
    scale = Math.max(0, scale - 0.05);
    out = oklabToRgb([target, a * scale, b * scale]);
  }
  return "#" + out.map(channel => Math.round(Math.min(1, Math.max(0, channel)) * 255).toString(16).padStart(2, "0")).join("") + (alpha === "ff" ? "" : alpha);
}

/**
 * The neutral tone of the Arc interface: tints and greys lose their hue so
 * the old teal-tinted surfaces read as plain greys; saturated status colors
 * (danger, warning, rarity) keep theirs. Takes and returns 8-digit hex.
 */
export function arcTone(digits) {
  const alpha = digits.slice(6, 8) || "ff";
  const rgb = [0, 2, 4].map(offset => parseInt(digits.slice(offset, offset + 2), 16) / 255);
  const [lightness, a, b] = rgbToOklab(rgb);
  if (Math.hypot(a, b) >= 0.1) return digits.slice(0, 6) + alpha;
  const out = oklabToRgb([lightness, a * 0.06, b * 0.06]);
  return out.map(channel => Math.round(Math.min(1, Math.max(0, channel)) * 255).toString(16).padStart(2, "0")).join("") + alpha;
}

const plain = digits => digits;

/** Rewrite the stylesheet's colors to tokens and append both themes. */
export function withDarkTheme(css, tone = plain) {
  const tokens = new Map();
  const tokenOf = hex => {
    const digits = expand(hex);
    if (!tokens.has(digits)) tokens.set(digits, "--k" + tokens.size.toString(36));
    return `var(${tokens.get(digits)})`;
  };
  // Rules of a game-styled component keep their colors in both themes.
  const themed = css.replace(RULE, (rule, selector, body) =>
    GAME_STYLED.test(selector) ? rule : selector + "{" + body.replace(VALUE, (_, value) => ":" + value.replace(HEX, tokenOf)) + "}",
  );
  const light = [...tokens].map(([source, name]) => { const digits = tone(source); return `${name}:#${digits.endsWith("ff") ? digits.slice(0, 6) : digits}`; }).join(";");
  const hex = digits => "#" + (digits.endsWith("ff") ? digits.slice(0, 6) : digits);
  const dark = [...tokens].map(([digits, name]) => `${name}:${hex(tone(expand(darkColor(digits))))}`).join(";") + ";color-scheme:dark";
  return `:root{${light}}${themed}:root[data-theme=dark]{${dark}}@media(prefers-color-scheme:dark){:root:not([data-theme=light]){${dark}}}`;
}
