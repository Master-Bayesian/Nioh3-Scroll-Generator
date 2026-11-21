/**
 * Production-bundle browser E2E for legal ordinary and hell equipment additions.
 * Reuses the expanded Arc bridge and production build without running its suite.
 * All save data is synthetic. Plans are reviewed and discarded, never committed.
 * Failure cases: stale success/error crosses a draft/source/mode boundary; wanted
 * pools omit eligible effects; skill gates effect search; queue follows a later
 * draft; or prepared identity differs from the reviewed candidate.
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, resolve } from "node:path";
import { parseArgs } from "node:util";
import {selectedLocales, workshopBundle} from '../ci/workshop-bundle.mjs';
import { chromium } from "playwright";
import { detectTextFit } from "../../apps/workshop/text-fit-detect.mjs";

const { values: options } = parseArgs({ options: { out: { type: "string" } } });
const repoRoot = resolve(import.meta.dirname, "../..");
const output = resolve(options.out || process.env.NIOH3_UI_OUTPUT ||
  resolve(repoRoot, "deliverables/hell-weapon-seed-search-20261116/browser"));
await mkdir(dirname(output), { recursive: true });
await mkdir(output); // A new run must not overwrite earlier evidence.
const sha256 = value => createHash("sha256").update(value).digest("hex");
const locales = selectedLocales(['zh-CN', 'en-US', 'ja-JP']);
const evidence = {
  schema: "nioh3-hell-equipment-seeds-browser-e2e/v1", status: "running", pass: false,
  repeat: "node tools/ui-audit/verify-hell-equipment-seeds.mjs --out <artifact-directory>",
  boundary: { productionBundle: true, syntheticBridge: true, gameProcess: false, realSave: false,
    saveCommit: false, nativeWindow: false, nativeGeneratorAcceptance: false, liveAddExtended: false },
  source: {}, locales, checks: [], screenshots: [], runs: [], failures: [],
};
function check(name, condition, details = null) {
  evidence.checks.push({ name, passed: Boolean(condition), details });
  assert.ok(condition, `${name}: ${JSON.stringify(details)}`);
}

const sharedSource = await readFile(resolve(repoRoot, "tools/ui-audit/verify-arc-expanded.mjs"), "utf8");
const bridgeStart = sharedSource.indexOf("const bridge = String.raw`") + "const bridge = String.raw`".length;
const bridgeEnd = sharedSource.indexOf("\n`;", bridgeStart);
assert.ok(bridgeStart > 25 && bridgeEnd > bridgeStart, "expanded Arc bridge extraction is intact");
const saveReference = {
  save_id: "arc-synthetic-save", path: "D:/synthetic/SAVEDATA00.BIN", account_id: "10001", save_slot: 0,
  summary: { character_level: 750, play_time_seconds: 590580, saved_at_unix: 1791859200, playthrough: 3 },
};
const baseBridge = sharedSource.slice(bridgeStart, bridgeEnd)
  .replace("${JSON.stringify(makeSaveReference())}", JSON.stringify(saveReference));
assert.ok(!baseBridge.includes("${"), "shared bridge has no unresolved template interpolation");
const fixtureBridge = String.raw`
(() => {
  const inherited = window.operations.execute;
  const clone = value => structuredClone(value);
  const state = window.__hell = {
    calls: [], searches: [], pending: [], nextBehavior: 'resolve', nextSearch: 1,
    nextPlan: 1, sourceVersion: 1, preview: null, prepared: null,
    failCharacterReads: false, characterReads: [],
  };
  const refs = [${JSON.stringify(saveReference)}, {
    ...${JSON.stringify(saveReference)}, save_id: 'hell-second-save', path: 'D:/synthetic/SAVEDATA01.BIN', save_slot: 1,
  }];
  const pool = (effect_id, group, star = false) => ({ effect_id, group, star, min: 1, max: 100, masks: [0, 0] });
  const rules = params => ({
    item_id: params.item_id, rarity: params.rarity, level: params.level, hell: !!params.hell,
    known: true, hell_capable: [171, 202, 457].includes(params.item_id), soul_core: false,
    roles: params.hell ? ['hell', 'random', 'random', 'random', 'random', 'random', 'grace']
      : ['random', 'random', 'random', 'random', 'random', 'random', params.item_id === 457 ? 'set' : 'grace'],
    random_pool: [pool(15, 1), pool(540, 2, true), pool(1, 3)],
    hell_pool: params.hell ? [pool(8489, 4), pool(8659, 5)] : [],
    graces: [5858], hell_skills: [41288, 56714], innate: [], set_effect: params.item_id === 457 ? 36356 : null,
  });
  const makeResponse = (params, number) => {
    const fx = (effect_id, value, role, star = false) => ({ effect_id, value, roll: 90, role, star });
    const wantedHell = (params.want || []).flat().find(id => [8489, 8659].includes(id));
    const outcomes = [0, 1].map(index => ({
      seed: 11000 + number * 2 + index,
      candidate_digest: (number * 2 + index).toString(16).padStart(64, '0'),
      effects: [
        ...(params.hell ? [fx(wantedHell || (index ? 8659 : 8489), 78 + index, 'hell')] : []),
        fx(15, 61 + index, 'random'), fx(540, 71 + index, 'random', true),
        fx(params.item_id === 457 ? 36356 : 5858, 0, params.item_id === 457 ? 'set' : 'grace'),
      ],
    }));
    return { item_id: params.item_id, rarity: params.rarity, level: params.level, generation_level: params.level,
      plus: params.plus, difficulty: params.difficulty, progress: clone(params.progress), hell: !!params.hell,
      generation_context: params.hell ? 'hell_conversion_zero_flags' : 'reward_zero_flags',
      hell_skills: params.hell ? [41288, 56714] : [], seeds: 65536, empty: 0, matches: 2, outcomes };
  };
  state.release = (kind = 'resolve') => {
    const pending = state.pending.splice(0);
    for (const item of pending) {
      if (kind === 'reject') item.reject(new Error('LATE_SYNTHETIC_SEARCH_FAILURE'));
      else item.resolve(clone(item.response));
    }
    return pending.length;
  };
  window.operations.execute = async command => {
    const { method, params = {} } = command;
    state.calls.push({ method, params: clone(params) });
    if (method === 'save.discover') return { saves: clone(refs) };
    if (method === 'save.inventory') {
      const result = await inherited(command);
      return { ...result, save_id: params.save_id, snapshot_id: 'hell-snapshot-' + state.sourceVersion,
        source_sha256: String(state.sourceVersion).repeat(64).slice(0, 64) };
    }
    if (method === 'save.character') {
      if (state.failCharacterReads) {
        state.characterReads.push({ params: clone(params), sourceVersion: state.sourceVersion, outcome: 'rejected' });
        throw new Error('SYNTHETIC_CHARACTER_READ_FAILURE');
      }
      const result = await inherited(command);
      const response = { ...result, save_id: params.save_id, equipment: result.equipment.slice(0, 4),
        currencies: params.save_id === 'hell-second-save' ? { amrita: 654321, gold: 123987 } : result.currencies,
        generation: { difficulty: 3, difficulties: [
          { difficulty: 2, progress: params.save_id === 'hell-second-save' ? [12, 11, 10, 9] : [2, 1, 1, 0] },
          { difficulty: 3, progress: params.save_id === 'hell-second-save' ? [13, 12, 11, 10] : [3, 2, 1, 0] },
        ] },
        source_sha256: String(state.sourceVersion).repeat(64).slice(0, 64) };
      state.characterReads.push({ params: clone(params), sourceVersion: state.sourceVersion, outcome: 'loaded', response: clone(response) });
      return response;
    }
    if (method === 'runtime.character_snapshot') {
      const result = await inherited(command);
      return { ...result, source: 'runtime', process_id: 1234, equipment: result.equipment.slice(0, 4) };
    }
    if (method === 'runtime.equipment_rules') return rules(params);
    if (method === 'runtime.equipment_seeds') {
      const behavior = state.nextBehavior; state.nextBehavior = 'resolve';
      const response = makeResponse(params, state.nextSearch++);
      if (behavior === 'empty') { response.matches = 0; response.outcomes = []; }
      if (behavior === 'all-empty') { response.matches = 0; response.empty = 65536; response.outcomes = []; }
      state.searches.push({ params: clone(params), response: clone(response), behavior });
      if (behavior === 'hold') return new Promise((resolve, reject) => state.pending.push({ resolve, reject, response }));
      if (behavior === 'reject') throw new Error('SYNTHETIC_SEARCH_FAILURE');
      return clone(response);
    }
    if (method === 'save.prepare_character_edit') {
      state.prepared = clone(params);
      state.preview = { added: (params.add || []).map((request, index) => {
        const search = state.searches.find(search => search.response.outcomes.some(outcome => outcome.candidate_digest === request.candidate_digest));
        const outcome = search?.response.outcomes.find(outcome => outcome.candidate_digest === request.candidate_digest);
        if (request.seeded && !outcome) throw new Error('E2E_UNKNOWN_CANDIDATE');
        return { slot_index: 1600 + index, seeded: !!request.seeded, after: {
          item_id: request.item_id, level: request.level, plus: request.plus, rarity: request.rarity,
          hell: !!request.hell, hell_skill: request.hell_skill || 0, seed: request.seed || 0,
          effects: clone(outcome?.effects || request.effects || []),
        } };
      }) };
      return { plan_id: 'hell-plan-' + state.nextPlan++, save_id: params.save_id,
        snapshot_id: params.snapshot_id, source_sha256: '1'.repeat(64), expires_in_seconds: 300,
        kind: 'character-edit', preview: clone(state.preview) };
    }
    if (/^(save.commit|runtime.character_edit|runtime.equipment_add_prepare|runtime.equipment_add_execute|runtime.live_batch_execute)$/.test(method))
      throw new Error('E2E_BOUNDARY: protected writes are forbidden');
    return inherited(command);
  };
})();
`;

async function routeEquipment(page) {
  const buttons = page.locator("nav.nav > button");
  for (let index = 0; index < await buttons.count(); index++) {
    if (await buttons.nth(index).isDisabled()) continue;
    await buttons.nth(index).click();
    if (await page.locator(".character-page").count()) return;
    const dialog = page.locator("dialog[open] header button").first();
    if (await dialog.count()) await dialog.click();
  }
  throw new Error("Equipment route is absent");
}
const editor = page => page.locator(".character-page:visible");
const panel = page => editor(page).locator(".seed-panel");
const field = (page, index) => editor(page).locator(".character-detail .character-fields input").nth(index);
const hellToggle = page => editor(page).locator('[data-action="toggle-hell-equipment"]');
const skill = page => editor(page).locator(".character-hell-row select");
async function chooseItem(page, id = 171) {
  await editor(page).locator(`.character-list [data-row="add-${id}"]`).click();
  await editor(page).locator('.character-edit-modes button').first().click();
  await editor(page).locator('[data-action="find-equipment-seeds"]:enabled').waitFor();
}
async function search(page, behavior = "resolve") {
  await page.evaluate(value => { window.__hell.nextBehavior = value; }, behavior);
  await editor(page).locator('[data-action="find-equipment-seeds"]').click();
  if (behavior === "hold") {
    await page.waitForFunction(() => window.__hell.pending.length > 0);
    await panel(page).locator(':scope[data-seed-phase="searching"]').waitFor();
  } else {
    await page.waitForFunction(() => {
      const phase = document.querySelector('.seed-panel')?.getAttribute('data-seed-phase');
      return ['results', 'empty', 'error'].includes(phase);
    });
  }
}
async function selectOutcome(page, index = 1) {
  await panel(page).locator('[role="option"]').nth(index).click();
  return panel(page).locator('[aria-selected="true"]').getAttribute("data-candidate-digest");
}
async function queueSnapshot(page) {
  return editor(page).locator(".character-queue li[data-equipment-request]").evaluateAll(rows => rows.map(row => ({
    request: JSON.parse(row.dataset.equipmentRequest),
    candidate: row.dataset.seededCandidate ? JSON.parse(row.dataset.seededCandidate) : null,
    text: row.textContent.trim(),
  })));
}
async function pickerIds(page) {
  const input = panel(page).locator(".effect-picker input");
  await panel(page).locator(".seed-want-label").click();
  await input.focus();
  const labels = await panel(page).locator(".effect-picker-list button").allTextContents();
  await input.press("Escape");
  return labels;
}
async function pickWanted(page, id) {
  const input = panel(page).locator(".effect-picker input");
  await panel(page).locator(".seed-want-label").click();
  await input.focus();
  await input.fill("0x" + id.toString(16).padStart(4, "0"));
  await panel(page).locator(".effect-picker-list li:not(:first-child) button").first().click();
}
async function capture(page, locale, name) {
  await page.waitForTimeout(100);
  const path = resolve(output, `${locale}-${name}.png`);
  await page.screenshot({ path });
  const detector = await page.evaluate(detectTextFit);
  const geometry = await editor(page).locator(".character-hell-row, .seed-panel").evaluateAll(elements => elements.map(element => {
    const r = element.getBoundingClientRect(); return { className: element.className, x: r.x, y: r.y, width: r.width, height: r.height };
  }));
  evidence.screenshots.push({ locale, name, path, sha256: sha256(await readFile(path)), detector, geometry });
  check(`${locale}/${name}: existing text-fit detector is clear`, detector.length === 0, detector);
}
async function assertInvalidated(page, name) {
  await page.waitForFunction(() => document.querySelector('.seed-panel')?.getAttribute('data-seed-phase') === 'idle');
  check(name, await panel(page).locator('[role="option"]').count() === 0 &&
    await panel(page).locator('[data-action="queue-equipment-seed"]').count() === 0);
}
async function verifyFailedSourceRead(page, locale, label, expectedAmrita) {
  const reload = editor(page).locator(".character-toolbar > button").first();
  await page.waitForFunction(() => window.__hell.characterReads.at(-1)?.outcome === 'rejected');
  await reload.waitFor({ state: "visible" });
  await page.waitForFunction(() => !document.querySelector('.character-toolbar > button')?.disabled);
  await panel(page).locator(".character-remove-note").waitFor();
  const before = await page.evaluate(() => window.__hell.searches.length);
  await panel(page).locator('[data-action="find-equipment-seeds"]').evaluate(button => button.click());
  await page.waitForTimeout(100);
  const snapshot = await page.evaluate(() => ({
    targetSave: document.querySelector('.character-page .save-picker select')?.value,
    cachedAmrita: document.querySelector('.character-currencies input')?.value,
    searchDisabled: document.querySelector('[data-action="find-equipment-seeds"]')?.disabled,
    searchRequests: window.__hell.searches.length,
    refreshInstruction: document.querySelector('.seed-panel .character-remove-note')?.textContent,
    errorNotice: document.querySelector('.character-page > .notice')?.textContent,
    reloadLabel: document.querySelector('.character-toolbar > button')?.textContent?.trim(),
    characterReads: structuredClone(window.__hell.characterReads),
  }));
  check(`${locale}: ${label} keeps cached character visible but disables search without an RPC`,
    snapshot.targetSave === "hell-second-save" && snapshot.cachedAmrita === String(expectedAmrita) &&
    snapshot.searchDisabled && snapshot.searchRequests === before, snapshot);
  check(`${locale}: ${label} shows the localized manual refresh instruction`,
    !!snapshot.refreshInstruction && !!snapshot.reloadLabel && snapshot.refreshInstruction.includes(snapshot.reloadLabel), snapshot.refreshInstruction);
  // Dismissing the optional global notice must not unlock a mismatched source.
  const closeNotice = editor(page).locator(".notice-error .notice-close");
  if (await closeNotice.count()) await closeNotice.click();
  check(`${locale}: ${label} remains blocked after dismissing the error notice`,
    await panel(page).locator('[data-action="find-equipment-seeds"]').isDisabled() &&
    await panel(page).locator(".character-remove-note").isVisible());
  await capture(page, locale, label);
  return snapshot;
}

async function runLocale(page, locale) {
  await routeEquipment(page);
  await editor(page).locator(".character-modes button").nth(1).click();
  await editor(page).locator('.save-picker select option[value="arc-synthetic-save"]').waitFor({ state: "attached" });
  await editor(page).locator(".save-picker select").selectOption("arc-synthetic-save");
  await editor(page).locator(".character-table-wrap").waitFor();
  await editor(page).locator('.character-sections [role="tab"]').nth(1).click();
  await chooseItem(page);
  check(`${locale}: legal add is the initial mode`, await panel(page).count() === 1);
  const ordinaryPool = await pickerIds(page);
  check(`${locale}: ordinary, star and grace choices are offered`, ordinaryPool.length === 5 && ordinaryPool.some(text => text.includes("✦")), ordinaryPool);
  await search(page);
  check(`${locale}: result selection is explicit`, await panel(page).getAttribute("data-seed-phase") === "results" &&
    await panel(page).locator('[data-action="queue-equipment-seed"]').isDisabled());
  await selectOutcome(page);
  await capture(page, locale, "ordinary-selected");
  await editor(page).locator('[data-action="queue-equipment-seed"]').click();
  const ordinaryQueue = await queueSnapshot(page);
  const ordinarySearch = await page.evaluate(() => window.__hell.searches.at(-1));
  check(`${locale}: ordinary seed, digest and generation identity are frozen`, ordinaryQueue[0].request.hell === false &&
    ordinaryQueue[0].request.seeded === true && ordinaryQueue[0].request.candidate_digest === ordinarySearch.response.outcomes[1].candidate_digest &&
    ordinaryQueue[0].request.generation_context === "reward_zero_flags" && ordinaryQueue[0].candidate.effects.length === 3, ordinaryQueue[0]);
  await hellToggle(page).check();
  await editor(page).locator('[data-action="find-equipment-seeds"]:enabled').waitFor();
  await assertInvalidated(page, `${locale}: hell route invalidates the ordinary result`);
  check(`${locale}: legal hell skill starts pending`, await skill(page).inputValue() === "0");
  const hellPool = await pickerIds(page);
  check(`${locale}: hell choices extend the ordinary pool`, hellPool.length === ordinaryPool.length + 2, hellPool);
  await pickWanted(page, 8489);
  await search(page);
  await selectOutcome(page);
  check(`${locale}: effect-only search succeeds before a skill is chosen`, await skill(page).inputValue() === "0" &&
    await panel(page).locator('[data-action="queue-equipment-seed"]').isDisabled());
  const searchesBeforeSkill = await page.evaluate(() => window.__hell.searches.length);
  await skill(page).selectOption("41288");
  check(`${locale}: choosing a skill preserves the selected effects`, await panel(page).getAttribute("data-seed-phase") === "selected" &&
    await page.evaluate(() => window.__hell.searches.length) === searchesBeforeSkill &&
    !(await panel(page).locator('[data-action="queue-equipment-seed"]').isDisabled()));
  await capture(page, locale, "hell-selected");
  await panel(page).locator('[data-action="queue-equipment-seed"]').click();
  const frozenQueue = await queueSnapshot(page);
  const hellSearch = await page.evaluate(() => window.__hell.searches.at(-1));
  check(`${locale}: queue freezes the reviewed skill and full selected outcome`, frozenQueue[1].request.hell_skill === 41288 &&
    frozenQueue[1].request.generation_context === "hell_conversion_zero_flags" &&
    JSON.stringify(frozenQueue[1].candidate) === JSON.stringify(hellSearch.response.outcomes[1]), frozenQueue[1]);
  await skill(page).selectOption("56714");
  await field(page, 0).fill("179");
  await field(page, 1).fill("23");
  await field(page, 2).fill("5");
  await editor(page).locator(".character-detail .character-fields select").selectOption("2");
  await hellToggle(page).uncheck();
  await editor(page).locator('[data-action="find-equipment-seeds"]:enabled').waitFor();
  await chooseItem(page, 202);
  check(`${locale}: later item/level/plus/rarity/difficulty/hell/skill drafts cannot mutate the queue`,
    JSON.stringify(await queueSnapshot(page)) === JSON.stringify(frozenQueue));
  check(`${locale}: only the now-invalid hell wanted choice is cleared`, await panel(page).locator(".seed-chip").count() === 0);
  await editor(page).locator(".character-queue button.primary").click();
  await editor(page).locator('[data-action="discard-character-plan"]').waitFor();
  const prepared = await page.evaluate(() => window.__hell.prepared);
  const preview = await page.evaluate(() => window.__hell.preview);
  check(`${locale}: prepare preserves every queued candidate field exactly`, JSON.stringify(prepared.add) ===
    JSON.stringify(frozenQueue.map(entry => entry.request)), prepared);
  check(`${locale}: plan preview preserves selected effects and skill`, preview.added[1].after.hell_skill === 41288 &&
    JSON.stringify(preview.added[1].after.effects) === JSON.stringify(frozenQueue[1].candidate.effects), preview);
  const planText = await editor(page).locator(".character-save-plan").textContent();
  const skillLabel = { "zh-CN": "地狱武技", "en-US": "Crucible Arts", "ja-JP": "地獄武技" }[locale];
  check(`${locale}: plan shows the localized skill label`, planText.includes(skillLabel), planText);
  await capture(page, locale, "hell-review");
  await editor(page).locator('[data-action="discard-character-plan"]').click();
  await editor(page).locator(".character-queue > button").last().click();
  await chooseItem(page, 457);
  const fixedPool = await pickerIds(page);
  check(`${locale}: applicable fixed-set wanted choice replaces grace`, fixedPool.length === ordinaryPool.length &&
    JSON.stringify(fixedPool) !== JSON.stringify(ordinaryPool), fixedPool);
  for (const [kind, id] of [["armor", 217], ["accessory", 1642]]) {
    await chooseItem(page, id);
    check(`${locale}: legal ${kind} does not offer an ineligible hell conversion`, await hellToggle(page).count() === 0);
  }
  await chooseItem(page);

  // Each generation input independently invalidates a completed selection.
  for (const [label, mutate] of [
    ["level", () => field(page, 0).fill("177")],
    ["plus", () => field(page, 1).fill("11")],
    ["rarity", () => field(page, 2).fill("3")],
    ["difficulty", () => editor(page).locator(".character-detail .character-fields select").selectOption("3")],
    ["wanted", () => pickWanted(page, 15)],
    ["item", () => chooseItem(page, 202)],
  ]) {
    await search(page); await selectOutcome(page); await mutate();
    await assertInvalidated(page, `${locale}: ${label} invalidates selected results`);
    await editor(page).locator('[data-action="find-equipment-seeds"]:enabled').waitFor();
  }
  check(`${locale}: still-valid ordinary wanted choice survives the item change`, await panel(page).locator(".seed-chip").count() === 1);

  // Earlier completions cannot replace a newer selection, including errors.
  for (const completion of ["resolve", "reject"]) {
    await search(page, "hold");
    await field(page, 1).fill(completion === "resolve" ? "12" : "13");
    await assertInvalidated(page, `${locale}: pending ${completion} invalidates after a condition change`);
    await search(page); const selected = await selectOutcome(page);
    await page.evaluate(kind => window.__hell.release(kind), completion);
    await page.waitForTimeout(100);
    check(`${locale}: late ${completion} cannot replace the newer selected result`,
      await panel(page).getAttribute("data-seed-phase") === "selected" &&
      await panel(page).locator('[aria-selected="true"]').getAttribute("data-candidate-digest") === selected &&
      !(await page.locator("body").textContent()).includes("LATE_SYNTHETIC_SEARCH_FAILURE"));
  }
  await search(page, "hold");
  await page.evaluate(() => { window.__hell.failCharacterReads = true; });
  await editor(page).locator(".save-picker select").selectOption("hell-second-save");
  await assertInvalidated(page, `${locale}: changing the selected save invalidates the pending search`);
  const failedSaveSwitch = await verifyFailedSourceRead(page, locale, "save-switch-read-failed", 123456);
  await page.evaluate(() => window.__hell.release());
  await assertInvalidated(page, `${locale}: a prior-save success remains invalid`);
  await page.evaluate(() => { window.__hell.failCharacterReads = false; });
  await editor(page).locator(".character-toolbar > button").first().click();
  await editor(page).locator('[data-action="find-equipment-seeds"]:enabled').waitFor();
  await search(page);
  const secondSaveSearch = await page.evaluate(() => window.__hell.searches.at(-1));
  check(`${locale}: manual successful refresh reenables search with the newly selected save's distinct progress`,
    JSON.stringify(secondSaveSearch.params.progress) === JSON.stringify([13, 12, 11, 10]), secondSaveSearch);
  await search(page, "hold");
  await page.evaluate(() => { window.__hell.sourceVersion++; window.__hell.failCharacterReads = true; });
  await editor(page).locator(".character-toolbar > button").first().click();
  const failedSourceRefresh = await verifyFailedSourceRead(page, locale, "source-refresh-read-failed", 654321);
  await page.evaluate(() => { window.__hell.failCharacterReads = false; });
  await editor(page).locator(".character-toolbar > button").first().click();
  await editor(page).locator('[data-action="find-equipment-seeds"]:enabled').waitFor();
  await page.evaluate(() => window.__hell.release("reject"));
  await assertInvalidated(page, `${locale}: refreshed source/snapshot rejects a late error`);
  await search(page);
  const refreshedSourceSearch = await page.evaluate(() => window.__hell.searches.at(-1));
  check(`${locale}: manual refresh after a source read failure restores current-save progress`,
    JSON.stringify(refreshedSourceSearch.params.progress) === JSON.stringify([13, 12, 11, 10]), refreshedSourceSearch);
  await search(page, "hold");
  await editor(page).locator(".character-edit-modes button").nth(1).click();
  await page.evaluate(() => window.__hell.release());
  await editor(page).locator(".character-edit-modes button").first().click();
  await assertInvalidated(page, `${locale}: legal/modded mode invalidates pending results`);
  await search(page, "hold");
  await editor(page).locator(".character-modes button").first().click();
  await page.evaluate(() => window.__hell.release("reject"));
  check(`${locale}: live add remains a distinct workflow`, await editor(page).locator(".seed-panel").count() === 0);
  await editor(page).locator(".character-modes button").nth(1).click();
  await editor(page).locator('.character-list [data-row="add-171"]').waitFor();
  await chooseItem(page);
  await assertInvalidated(page, `${locale}: returning to save mode cannot resurrect prior results`);
  for (const behavior of ["empty", "all-empty", "reject"]) {
    await search(page, behavior);
    check(`${locale}: ${behavior} never enables queueing`, await panel(page).locator('[data-action="queue-equipment-seed"]').count() === 0 ||
      await panel(page).locator('[data-action="queue-equipment-seed"]').isDisabled());
  }
  const bridge = await page.evaluate(() => ({ calls: window.__hell.calls, searches: window.__hell.searches,
    characterReads: window.__hell.characterReads, prepared: window.__hell.prepared,
    preview: window.__hell.preview, pending: window.__hell.pending.length }));
  const forbidden = bridge.calls.filter(call => /^(save.commit|runtime.character_edit|runtime.equipment_add_prepare|runtime.equipment_add_execute|runtime.live_batch_execute)$/.test(call.method));
  check(`${locale}: no game or save write was dispatched`, forbidden.length === 0, forbidden);
  evidence.runs.push({ locale, ordinaryPool, hellPool, fixedPool, ordinarySearch, hellSearch,
    ordinaryQueue, frozenQueue, prepared, preview, planText,
    failedSaveSwitch, failedSourceRefresh, secondSaveSearch, refreshedSourceSearch, bridge });
}

let server;
let browser;
try {
  evidence.source.head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).trim();
  for (const path of ["apps/workshop/CharacterEditor.tsx", "apps/workshop/equipment-seed-state.ts",
    "apps/workshop/public-errors.ts", "apps/workshop/ui-locales.json",
    "packages/contracts/protected-request.schema.json", "packages/contracts/protected-response.schema.json",
    "tools/ui-audit/verify-hell-equipment-seeds.mjs", "tools/ui-audit/verify-arc-expanded.mjs"]) {
    const source = await readFile(resolve(repoRoot, path));
    evidence.source[path] = sha256(source);
    const snapshot = resolve(output, "source-snapshot", path);
    await mkdir(dirname(snapshot), { recursive: true });
    await writeFile(snapshot, source);
  }
  const bundle = await readFile(workshopBundle(repoRoot), "utf8");
  evidence.source.bundleSha256 = sha256(bundle);
  await writeFile(resolve(output, "production-bundle.html"), bundle, "utf8");
  evidence.source.sharedBridgeSha256 = sha256(baseBridge);
  const html = bundle.replace("</head>", `<script>${baseBridge}\n${fixtureBridge}</script></head>`);
  server = createServer((_request, response) => { response.writeHead(200, { "content-type": "text/html; charset=utf-8" }); response.end(html); });
  await new Promise(done => server.listen(0, "127.0.0.1", done));
  browser = await chromium.launch({ headless: true });
  for (const locale of locales) {
    const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
    const page = await context.newPage();
    page.setDefaultTimeout(10000);
    page.on("pageerror", error => evidence.failures.push({ locale, phase: "browser", message: String(error) }));
    await page.addInitScript(value => { localStorage.setItem("nioh3-ui-locale", value); }, locale);
    try {
      console.log(`RUN ${locale}`);
      await page.goto(`http://127.0.0.1:${server.address().port}`, { waitUntil: "domcontentloaded" });
      await page.locator("#root .shell").waitFor();
      await runLocale(page, locale);
    } catch (error) {
      console.error(`${locale}: ${String(error)}`);
      evidence.failures.push({ locale, phase: "scenario", message: String(error), stack: error.stack });
      const path = resolve(output, `${locale}-failure.png`);
      await page.screenshot({ path }).catch(() => {});
      evidence.screenshots.push({ locale, name: "failure", path });
      evidence.runs.push({ locale, failed: true, bridge: await page.evaluate(() => ({ calls: window.__hell?.calls,
        searches: window.__hell?.searches, prepared: window.__hell?.prepared })).catch(() => null) });
    } finally { await context.close(); }
  }
} catch (error) {
  evidence.failures.push({ phase: "fatal", message: String(error), stack: error.stack });
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) await new Promise(done => server.close(done));
  evidence.pass = evidence.failures.length === 0 && evidence.checks.every(item => item.passed);
  evidence.status = evidence.pass ? "passed" : "failed";
  const path = resolve(output, "verify-hell-equipment-seeds.json");
  await writeFile(path, JSON.stringify(evidence, null, 2) + "\n", "utf8");
  console.log(JSON.stringify({ status: evidence.status, checks: evidence.checks.length,
    failures: evidence.failures.length, artifact: path }, null, 2));
  if (!evidence.pass) process.exitCode = 1;
}
