// One-off rewrite: text boxes grow with their content instead of clipping it.
//   node tools/relax_fixed_text_boxes.mjs <css>... [--dry]
// - `height:Npx` becomes `min-height:Npx`, except on form controls, buttons (single-line by design), media and
//   scroll containers (rules that also set overflow auto/scroll keep a fixed box).
// - px tracks in grid-template-rows / grid-auto-rows become minmax(Npx,auto).
import fs from 'node:fs';

const args = process.argv.slice(2);
const dry = args.includes('--dry');
const fixedControl = /(^|[\s>+~(,])(input|select|textarea|img|svg|canvas|video|hr|progress|iframe|button)\b|::?-webkit-|::?(before|after)|\.(brand-mark|app-icon|rarity-dot|drag-grip|lock-mark|switch|visually-hidden|[\w-]*button|[\w-]*toggle)\b|\[type=/i;

function lastCompound(selector) {
  return selector.split(',').map((s) => s.trim().split(/\s+|>|\+|~/).filter(Boolean).pop() || '');
}

for (const file of args.filter((a) => a !== '--dry')) {
  const source = fs.readFileSync(file, 'utf8');
  let changed = 0;
  const out = source.replace(/([^{}]+)\{([^{}]*)\}/g, (whole, selector, body) => {
    if (/^\s*@/.test(selector) || /^\s*(from|to|\d+%)\s*$/.test(selector.trim())) return whole;
    let next = body;
    const scrolls = /overflow(-y)?:\s*(auto|scroll)/.test(body);
    // Both px width and height set: a fixed-size box such as an icon.
    const sized = /(^|[;\s])width:\s*\d+(\.\d+)?px/.test(body) && /(^|[;\s])height:\s*\d+(\.\d+)?px/.test(body);
    const controls = sized || lastCompound(selector).every((c) => fixedControl.test(' ' + c));
    if (!scrolls && !controls) {
      next = next.replace(/(^|[;\s])height:\s*(\d+(?:\.\d+)?px)(\s*!important)?/g, (m, lead, px, imp) => {
        changed++;
        return `${lead}min-height:${px}${imp || ''}`;
      });
    }
    next = next.replace(/(grid-template-rows|grid-auto-rows):([^;}]*)/g, (m, prop, value) => {
      if (!/\d+px/.test(value) || /minmax\(\s*\d+px\s*,\s*auto\)/.test(value)) return m;
      // A px track already inside minmax()/fit-content() keeps its bounds.
      const relaxed = value.replace(/(^|[\s(,])(\d+(?:\.\d+)?px)(?=[\s),;]|$)/g, (t, lead, px, offset, all) =>
        lead === '(' && /(minmax|fit-content)$/.test(all.slice(0, offset)) ? t : `${lead}minmax(${px},auto)`);
      if (relaxed !== value) changed++;
      return `${prop}:${relaxed}`;
    });
    return next === body ? whole : `${selector}{${next}}`;
  });
  console.log(`${file}: ${changed} declarations relaxed`);
  if (!dry) fs.writeFileSync(file, out);
}
