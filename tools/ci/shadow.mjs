import ts from 'typescript';
import {existsSync, readFileSync} from 'node:fs';
import {dirname, relative, resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {appendGithubOutput, balance, ensure, loadPlan, printFailure, readJson, seal, trackedFiles, verifySource, writeJson} from './core.mjs';

/** @typedef {{mode:'Full', reasons:string[], selected:string[]} | {mode:'Affected', reasons:string[], selected:string[]} | {mode:'Blocked', reasons:string[], selected:string[]}} Decision */

const fullFiles = new Set(['package.json', 'package-lock.json', 'tsconfig.json', 'tsconfig.e2e.json', 'tsconfig.ci.json', 'eslint.config.mjs', '.gitattributes', '.gitignore', 'rust-toolchain.toml']);
const fullPrefixes = ['.github/', 'tools/', 'spec/', 'data/', 'crates/', 'research/', 'packages/contracts/', 'assets/', 'packaging/', 'test_fixtures/', 'test-fixtures/', 'third_party/'];
const sourceFile = file => /\.(?:[cm]?js|tsx?)$/.test(file);

function dependencyGraph(root, files) {
  const known = new Set(files);
  const graph = new Map();
  const unsafe = new Set();
  const filesystem = new Set();
  const configPath = resolve(root, 'tsconfig.json');
  const config = ts.readConfigFile(configPath, ts.sys.readFile);
  ensure(!config.error, 'CI_GRAPH_CONFIG', 'Cannot read the TypeScript configuration.');
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root);
  ensure(parsed.errors.length === 0, 'CI_GRAPH_CONFIG', 'Cannot resolve TypeScript compiler options.');
  for (const file of files.filter(sourceFile)) {
    const absolute = resolve(root, file);
    const tree = ts.createSourceFile(absolute, readFileSync(absolute, 'utf8'), ts.ScriptTarget.Latest, true);
    if(tree.parseDiagnostics.length)unsafe.add(file);
    const dependencies = new Set();
    const record = specifier => {
      if (/^(?:node:)?fs(?:\/promises)?$/.test(specifier)) filesystem.add(file);
      const resolved = ts.resolveModuleName(specifier, absolute, parsed.options, ts.sys).resolvedModule?.resolvedFileName;
      if (resolved) {
        const local = relative(root, resolved).replaceAll('\\', '/');
        if (known.has(local)) dependencies.add(local);
        return;
      }
      if (!specifier.startsWith('.')) return; // Package changes are full triggers.
      const candidate = relative(root, resolve(dirname(absolute), specifier)).replaceAll('\\', '/');
      const found = [candidate, ...['.ts', '.tsx', '.mjs', '.js', '.json', '/index.ts', '/index.tsx'].map(extension => candidate + extension)].find(path => known.has(path));
      if (found) dependencies.add(found);
      else unsafe.add(file);
    };
    const visit = node => {
      if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) record(node.moduleSpecifier.text);
      if (ts.isCallExpression(node) && (node.expression.kind === ts.SyntaxKind.ImportKeyword || ts.isIdentifier(node.expression) && node.expression.text === 'require')) {
        if (node.arguments.length === 1 && ts.isStringLiteralLike(node.arguments[0])) record(node.arguments[0].text);
        else unsafe.add(file);
      }
      ts.forEachChild(node, visit);
    };
    visit(tree);
    graph.set(file, dependencies);
  }
  return {graph, unsafe, filesystem};
}

function closure(graph, roots) {
  const visited = new Set();
  const pending = [...roots];
  while (pending.length) {
    const file = pending.pop();
    if (visited.has(file)) continue;
    visited.add(file);
    pending.push(...(graph.get(file) ?? []));
  }
  return visited;
}

export function selectFrontend(root, plan) {
  const tasks = plan.inventory.tasks.filter(task => task.lane === 'frontend');
  const all = tasks.map(task => task.id);
  const full = reason => ({decision: /** @type {Decision} */ ({mode: 'Full', reasons: [reason], selected: all}), graph: {status: 'NotRequired'}, matches: {}});
  if (plan.comparison === 'FullOnly') return full('Integration events require a complete normal profile.');
  if (!plan.identity.mergeBaseSha) return full('Merge-base history is unavailable.');
  if (!plan.changed.length) return full('No comparison diff is available; retain the full reference.');
  const paths = [...new Set(plan.changed.flatMap(change => [change.oldPath, change.newPath]))];
  if (plan.changed.some(change => change.status !== 'M')) return full('Structural/add/delete/rename changes conservatively use the full profile.');
  if (paths.some(path => fullFiles.has(path) || fullPrefixes.some(prefix => path.startsWith(prefix)) || path.endsWith('/build.mjs') || !sourceFile(path))) return full('A non-imported, configuration, cross-language or unclassified input changed.');
  const files = trackedFiles(root);
  if (paths.some(path => !files.includes(path))) return full('A changed source file is not in the complete tracked inventory.');
  let data;
  try { data = dependencyGraph(root, files); }
  catch (error) { return full(`Dependency graph unavailable: ${String(error.message)}`); }
  const selected = [];
  const matches = {};
  for (const task of tasks) {
    const dependencies = closure(data.graph, task.roots);
    if ([...dependencies].some(file => data.unsafe.has(file))) return full('A consuming root contains an unresolved or nonliteral dependency.');
    const hits = paths.filter(path => dependencies.has(path));
    // Standalone browser fixtures have build/process/file inputs beyond imports.
    if (task.kind === 'browser') hits.push(...paths.filter(path => path.startsWith('apps/') || path.startsWith('packages/')));
    if (task.kind === 'node-test' && [...dependencies].some(file => data.filesystem.has(file))) hits.push('conservative filesystem-consumer inclusion');
    if (hits.length) { selected.push(task.id); matches[task.id] = [...new Set(hits)]; }
  }
  return {decision: /** @type {Decision} */ ({mode: 'Affected', reasons: ['Transitive literal imports plus explicit conservative browser/filesystem inputs; shadow execution only.'], selected}), graph: {status: 'Computed', files: data.graph.size, changed: paths}, matches};
}

export function createShadow(root, plan) {
  const selection = selectFrontend(root, plan);
  /** @type {Decision} */ const decision = selection.decision;
  switch (decision.mode) {
    case 'Full':
    case 'Affected': break;
    case 'Blocked': throw new Error('A blocked selection cannot execute.');
  }
  const timings = readJson(resolve(root, 'tools/ci/timings.json'));
  const tasks = plan.inventory.tasks.filter(task => decision.selected.includes(task.id));
  const groups = balance(tasks, 4, timings.tasks).map(shard => ({id: `shadow-${shard.id}`, lane: 'shadow', shard: shard.id, tasks: shard.tasks, estimatedMs: shard.estimatedMs}));
  return seal({schema: 'nioh3-ci-shadow/v1', sourceId: plan.sourceId, planDigest: plan.digest, identity: plan.identity, comparison: plan.comparison, decision, groups, graph: selection.graph, matches: selection.matches, changes: plan.changed, pruningEnabled: false});
}

async function main() {
  const {values} = parseArgs({options: {root: {type: 'string'}, plan: {type: 'string'}, out: {type: 'string'}}});
  const root = resolve(values.root ?? process.cwd());
  ensure(values.plan && values.out, 'CI_ARGUMENTS', '--plan and --out are required.');
  const plan = loadPlan(resolve(values.plan));
  verifySource(root, plan);
  const shadow = createShadow(root, plan);
  writeJson(resolve(values.out, 'shadow.json'), shadow);
  appendGithubOutput('shadow', {include: shadow.groups.map(group => ({id: group.id}))});
  console.log(JSON.stringify({mode: shadow.decision.mode, selected: shadow.decision.selected.length, fullFrontend: plan.inventory.tasks.filter(task => task.lane === 'frontend').length, pruningEnabled: false}));
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  try { await main(); } catch (error) { printFailure(error); }
}
