/** Exercise the actual configured lint rules with valid, invalid, and disabled-rule controls. */
import assert from 'node:assert/strict';
import {mkdir, mkdtemp, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {ESLint} from 'eslint';
import configuration from '../eslint.config.mjs';

const buildRoot = process.env.NIOH3_BUILD_ROOT || (process.env.RUNNER_TEMP
  ? path.join(process.env.RUNNER_TEMP, 'nioh3-quality') : 'D:/Nioh3_v080_deliverables');
const tempRoot = path.join(buildRoot, 'tmp');
await mkdir(tempRoot, {recursive:true});
const fixture = await mkdtemp(path.join(tempRoot, 'lint-controls-'));
const output = process.env.NIOH3_QUALITY_EVIDENCE || path.join(buildRoot, 'deliverables/quality-controls');
await mkdir(output, {recursive:true});
await mkdir(path.join(fixture, 'apps/desktop/src'), {recursive:true});
await writeFile(path.join(fixture, 'tsconfig.json'), JSON.stringify({compilerOptions:{target:'ES2022',module:'ESNext',moduleResolution:'Bundler',strict:true,noEmit:true,lib:['ES2022'],types:[]},include:['apps/**/*.ts']}));
const cases = [
  {name:'unobserved-operation',rule:'@typescript-eslint/no-floating-promises',
    valid:'async function operation() { return 1; } async function caller() { return await operation(); } void caller();',
    invalid:'async function operation() { return 1; } operation();'},
  {name:'promise-as-state-condition',rule:'@typescript-eslint/no-misused-promises',
    valid:'async function caller() { if (await Promise.resolve(true)) return 1; return 0; } void caller();',
    invalid:'function caller() { if (Promise.resolve(true)) return 1; return 0; } caller();'},
  {name:'await-nonoperation',rule:'@typescript-eslint/await-thenable',
    valid:'async function caller() { return await Promise.resolve(1); } void caller();',
    invalid:'async function caller() { return await 1; } void caller();'},
  {name:'default-hides-unhandled-state',rule:'@typescript-eslint/switch-exhaustiveness-check',
    valid:'function step(state: "idle" | "running") { switch (state) { case "idle": return 0; case "running": return 1; default: return 2; } } void step;',
    invalid:'function step(state: "idle" | "running") { switch (state) { case "idle": return 0; default: return 2; } } void step;'},
];
for (const control of cases) for (const kind of ['valid','invalid'])
  await writeFile(path.join(fixture, `apps/desktop/src/${control.name}-${kind}.ts`), `export {};\n${control[kind]}\n`);
// The rule/plugin objects are the production configuration. Only the temporary
// TypeScript project root changes, so parser errors cannot masquerade as lint success.
const config = configuration.map(entry => entry.languageOptions?.parserOptions
  ? {...entry,languageOptions:{...entry.languageOptions,parserOptions:{...entry.languageOptions.parserOptions,tsconfigRootDir:fixture}}}
  : entry);
const eslint = new ESLint({cwd:fixture,overrideConfigFile:true,overrideConfig:config});
const evidence = {scope:'Actual production lint configuration; real TypeScript projects; no app mutation',fixture,cases:[]};
for (const control of cases) {
  const file = kind => path.join(fixture, `apps/desktop/src/${control.name}-${kind}.ts`);
  const [valid] = await eslint.lintFiles([file('valid')]);
  const [invalid] = await eslint.lintFiles([file('invalid')]);
  assert.equal(valid.errorCount,0,`${control.name}: valid control must pass: ${JSON.stringify(valid.messages)}`);
  assert.ok(!invalid.messages.some(m=>m.fatal),`${control.name}: parser failure is not a rule rejection`);
  assert.ok(invalid.messages.some(m=>m.ruleId===control.rule && m.severity===2),`${control.name}: expected rule must reject the defect`);
  const disabled = new ESLint({cwd:fixture,overrideConfigFile:true,overrideConfig:[...config,{files:['apps/**/*.ts'],rules:{[control.rule]:'off'}}]});
  const [mutant] = await disabled.lintFiles([file('invalid')]);
  assert.equal(mutant.errorCount,0,`${control.name}: disabling the target rule must admit that same defect`);
  evidence.cases.push({name:control.name,rule:control.rule,validAccepted:true,invalidRejected:true,disabledRuleAdmitsDefect:true,messages:invalid.messages});
}
evidence.pass = true;
await writeFile(path.join(output,'lint-controls.json'),JSON.stringify(evidence,null,2)+'\n');
console.log(JSON.stringify({pass:true,rules:cases.length,checks:cases.length*3,evidence:path.join(output,'lint-controls.json')}));
