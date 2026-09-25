// Compare the machine code around one hook site in two `.text` section dumps.
//
// Usage:
//   node tools/compare_text_sites.mjs <old.text.bin> <new.text.bin> \
//     <old_rva> <new_rva> [before=0x200] [after=0x100] [text_rva=0x1000]
//
// Prints every differing byte run relative to the site with a little context,
// so a reviewer can confirm that the only differences are rel32 call/jump
// displacements and RIP-relative operands (a relocated but otherwise identical
// function body). Read-only: it never touches a process or an executable.
import { readFileSync } from "node:fs";

const [oldPath, newPath, oldArg, newArg, beforeArg, afterArg, textArg] =
  process.argv.slice(2);
if (!oldPath || !newPath || !oldArg || !newArg) {
  console.error(
    "usage: compare_text_sites.mjs <old.text.bin> <new.text.bin> <old_rva> <new_rva> [before] [after] [text_rva]",
  );
  process.exit(2);
}
const oldText = readFileSync(oldPath);
const newText = readFileSync(newPath);
const oldRva = Number(oldArg);
const newRva = Number(newArg);
const before = Number(beforeArg ?? 0x200);
const after = Number(afterArg ?? 0x100);
const textRva = Number(textArg ?? 0x1000);

const window = (buffer, rva, start, end) =>
  buffer.subarray(rva - textRva + start, rva - textRva + end);
const hex = (bytes) =>
  [...bytes].map((value) => value.toString(16).padStart(2, "0")).join(" ");
const offset = (value) =>
  `${value < 0 ? "-" : "+"}0x${Math.abs(value).toString(16)}`;

const a = window(oldText, oldRva, -before, after);
const b = window(newText, newRva, -before, after);
const runs = [];
for (let index = 0; index < a.length; index++) {
  if (a[index] === b[index]) continue;
  const position = index - before;
  const last = runs.at(-1);
  if (last && position - last[1] <= 1) last[1] = position;
  else runs.push([position, position]);
}
const differing = runs.reduce((sum, [start, end]) => sum + end - start + 1, 0);
console.log(
  `old 0x${oldRva.toString(16)} / new 0x${newRva.toString(16)}, window ${offset(-before)}..${offset(after)}: ${differing} differing bytes in ${runs.length} runs`,
);
for (const [start, end] of runs) {
  console.log(
    `  ${offset(start)}..${offset(end)}  old: ${hex(window(oldText, oldRva, start - 6, end + 4))}  new: ${hex(window(newText, newRva, start - 6, end + 4))}`,
  );
}
