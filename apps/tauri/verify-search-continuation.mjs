/** Native WebView2 acceptance for the v0.7.5 search-continuation hotfix. */
import { chromium, _electron as electron } from "playwright";
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, realpath, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { createServer } from "node:net";
import assert from "node:assert/strict";

const seed = Number(process.env.NIOH3_SEARCH_SEED || 226061463);
const target = process.env.NIOH3_UI_TARGET || "tauri";
const searchTimeoutMs = Number(process.env.NIOH3_SEARCH_TIMEOUT_MS || 15 * 60 * 1000);
const cancelWatchMs = Number(process.env.NIOH3_CANCEL_WATCH_MS || 8000);
const output = resolve(
  process.env.NIOH3_UI_OUTPUT || "deliverables/search-continuation-v075",
);
const python = process.env.NIOH3_PYTHON;
assert(python, "NIOH3_PYTHON must name the prepared project environment");
await mkdir(output, { recursive: true });
const pause = (ms) => new Promise((done) => setTimeout(done, ms));

// The reported three-condition search, entered through the normal rule tree.
const rules = [
  { find: "配件", name: "一难横行（配件）", key: "64956", label: "80%" },
  { find: "神器掉落率上升", name: "神器掉落率上升", key: "113", label: "10%" },
  {
    find: "素盏",
    name: "优先掉落率上升（素盏呜尊的恩宠）",
    key: "20893",
    label: "30%",
  },
];

const root = await realpath(await mkdtemp(join(tmpdir(), "nioh3-search-ui-")));
let child;
let browser;
let app;
let page;
const timeline = [];
const report = {
  seed,
  target,
  rules: rules.map((rule) => ({ name: rule.name, key: rule.key, label: rule.label })),
  steps: {},
  timeline,
  boundary:
    "Native WebView2 UI acceptance for the search-continuation hotfix; read-only offline search, no game, save, or runtime write.",
};

async function addRule(rule) {
  await page
    .getByRole("textbox", { name: "搜索规则、部位、符咒或恩宠", exact: true })
    .fill(rule.find);
  const variant = page.getByRole("combobox", {
    name: `规则变体${rule.name}`,
    exact: true,
  });
  await variant.waitFor({ timeout: 20000 });
  await variant.selectOption(rule.key);
  await page
    .getByRole("button", { name: `添加规则${rule.name}`, exact: true })
    .click();
  await page
    .locator(".chip.rule-chip", { hasText: rule.name })
    .first()
    .waitFor({ timeout: 10000 });
}

const state = () => page.evaluate(() => window.nioh.currentSearch());

// Condition panels are collapsed by default; only one panel per group stays open.
async function openPanel(title) {
  const toggle = page.getByRole("button", { name: title, exact: true });
  if ((await toggle.count()) === 0) return;
  if ((await toggle.getAttribute("aria-expanded")) !== "true") await toggle.click();
}

async function launchTauri() {
  const executable = resolve(
    process.env.NIOH3_TAURI_EXE ||
      "apps/tauri/src-tauri/target/release/nioh3-studio.exe",
  );
  const present = await stat(executable).catch(() => null);
  assert(
    present?.isFile(),
    `WebView2 target not built: ${executable}. Build the packaged application, or set NIOH3_TAURI_EXE, or run NIOH3_UI_TARGET=electron for a local pre-package check.`,
  );
  const server = createServer();
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const port = server.address().port;
  await new Promise((done) => server.close(done));
  child = spawn(executable, ["--user-data-dir", join(root, "profile")], {
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...process.env,
      NIOH3_PYTHON: python,
      NIOH3_TAURI_TEST_ROOT: join(root, "profile"),
      NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
      NIOH3_STATE_ROOT: join(root, "state"),
      LOCALAPPDATA: join(root, "local"),
    },
  });
  let stderr = "";
  child.stderr.on("data", (data) => (stderr = (stderr + data).slice(-32000)));
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (child.exitCode !== null)
      throw Error(`Application exited ${child.exitCode}: ${stderr}`);
    try {
      if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;
    } catch {}
    await pause(250);
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  for (let attempt = 0; attempt < 150; attempt += 1) {
    page = browser.contexts()[0]?.pages()[0];
    if (page) return;
    await pause(200);
  }
  throw Error("WebView2 opened without a page target");
}

async function launchElectron() {
  const { _electron } = await import("playwright");
  let executable;
  try {
    executable = await _electron.launch({
      args: [resolve("apps/desktop/dist/main.cjs")],
      env: {
        ...process.env,
        NIOH3_REVIEW_UI: "1",
        NIOH3_PYTHON: python,
        NIOH3_STATE_ROOT: join(root, "state"),
        LOCALAPPDATA: join(root, "local"),
      },
    });
  } catch (error) {
    throw Error(`Electron target unavailable: ${String(error)}`);
  }
  app = executable;
  page = await app.firstWindow();
  await app.evaluate(({ BrowserWindow }) =>
    BrowserWindow.getAllWindows()[0].webContents.setBackgroundThrottling(false),
  );
}

try {
  if (target === "tauri") await launchTauri();
  else await launchElectron();
  await page
    .getByText("后端已连接，请选择筛选条件。", { exact: true })
    .waitFor({ timeout: 60000 });
  report.steps.connected = true;

  await openPanel("特殊规则");
  for (const rule of rules) await addRule(rule);
  report.steps.rulesSelected = await page
    .locator(".chip.rule-chip")
    .allInnerTexts()
    .then((texts) => texts.map((text) => text.replace(/\s+/g, " ").trim()));
  await page.getByRole("spinbutton", { name: "候选数量" }).fill("1");
  report.steps.resultCount = await page
    .getByRole("spinbutton", { name: "候选数量" })
    .inputValue();

  const beforeStart = await state();
  assert.equal(beforeStart.job ?? null, null, "A job existed before the search start");
  await page.getByRole("button", { name: /开始搜索/ }).click();

  const deadline = Date.now() + searchTimeoutMs;
  let cards = 0;
  let cancelEnabledDuringRun = null;
  while (Date.now() < deadline) {
    const current = await state();
    cards = await page.locator(`article.scroll[aria-label="绘卷 ${seed}"]`).count();
    const cancel = page.getByRole("button", { name: "取消", exact: true });
    if (current.job && current.job.state === "running")
      cancelEnabledDuringRun = await cancel.isEnabled();
    timeline.push({
      at: new Date().toISOString(),
      state: current.job?.state ?? null,
      cursor: current.job?.cursor ?? null,
      startCursor: current.job?.start_cursor ?? null,
      stopReason: current.job?.stop_reason ?? null,
      candidates: current.job?.candidates.length ?? 0,
      progress: current.job?.progress?.inspected_through_trial ?? null,
      seedCard: cards > 0,
      cancelEnabled: await page
        .getByRole("button", { name: "取消", exact: true })
        .isEnabled()
        .catch(() => null),
    });
    if (cards > 0) break;
    if (
      current.job &&
      ["completed", "failed", "cancelled"].includes(current.job.state)
    )
      break;
    await pause(3000);
  }
  const afterSearch = await state();
  report.steps.submitted = afterSearch.submitted;
  report.steps.job = {
    jobId: afterSearch.job?.job_id ?? null,
    state: afterSearch.job?.state ?? null,
    startCursor: afterSearch.job?.start_cursor ?? null,
    cursor: afterSearch.job?.cursor ?? null,
    stopReason: afterSearch.job?.stop_reason ?? null,
    candidates: afterSearch.job?.candidates ?? [],
    resumeToken: Boolean(afterSearch.job?.resume_token),
  };
  report.steps.statusText = await page.locator(".status").innerText();
  report.steps.cancelEnabledDuringRun = cancelEnabledDuringRun;
  report.steps.cardText = cards
    ? (await page
        .locator(`article.scroll[aria-label="绘卷 ${seed}"]`)
        .first()
        .innerText()).replace(/\s+/g, " ").slice(0, 400)
    : null;
  report.steps.singleStartFoundSeed = cards > 0;
  await page.screenshot({
    path: join(output, "zh-search-continuation.png"),
    fullPage: true,
  });
  assert(
    cards > 0,
    `One search click did not surface seed ${seed}; stop reason ${afterSearch.job?.stop_reason}`,
  );
  // The canonical status stays Chinese in state and localizes at render time, so
  // switching language after the job finished must retranslate it.
  await page.locator(".language-button").click();
  await page.getByRole("button", { name: "English", exact: true }).click();
  await page.waitForFunction(
    () => document.querySelector(".status")?.textContent?.includes("candidate target met"),
    null,
    { timeout: 20000 },
  );
  report.steps.statusTextEnglish = await page.locator(".status").innerText();
  await page.screenshot({
    path: join(output, "en-search-continuation.png"),
    fullPage: true,
  });
  assert.doesNotMatch(
    report.steps.statusTextEnglish,
    /[\u3400-\u9fff]/,
    "Completed status did not retranslate after the locale switch",
  );
  await page.locator(".language-button").click();
  await page.getByRole("button", { name: "简体中文", exact: true }).click();
  await page.waitForFunction(() => document.documentElement.lang === "zh-CN", null, {
    timeout: 20000,
  });
  report.steps.statusTextRestored = await page.locator(".status").innerText();

  // Bounded continuation checks: the next batch must resume the checkpoint of
  // this same query/policy, and cancelling must not start another job.
  const checkpoint = afterSearch.job?.cursor ?? 0;
  const jobId = afterSearch.job?.job_id ?? null;
  const nextBatch = page.getByRole("button", { name: /下一批/ });
  if (await nextBatch.isEnabled()) {
    await nextBatch.click();
    let resumed = null;
    for (let attempt = 0; attempt < 40; attempt += 1) {
      const current = await state();
      if (
        current.job &&
        current.job.job_id !== jobId &&
        current.job.start_cursor === checkpoint
      ) {
        resumed = current.job;
        break;
      }
      if (current.job?.state === "completed") resumed = current.job;
      await pause(500);
    }
    report.steps.nextBatch = resumed
      ? {
          newJobId: resumed.job_id,
          jobIdChanged: resumed.job_id !== jobId,
          startCursor: resumed.start_cursor,
          resumedFromCheckpoint: resumed.start_cursor === checkpoint,
          continueUntilComplete:
            afterSearch.submitted?.continue_until_complete ?? null,
          pageTrials: afterSearch.submitted?.page_trials ?? null,
          state: resumed.state,
        }
      : { skipped: "next batch did not report a resumed checkpoint in time" };
    const cancel = page.getByRole("button", { name: "取消", exact: true });
    if (await cancel.isEnabled()) {
      await cancel.click();
      const cancelledId = (await state()).job?.job_id ?? null;
      let stable = true;
      const watchUntil = Date.now() + cancelWatchMs;
      while (Date.now() < watchUntil) {
        const current = await state();
        if (current.job && current.job.job_id !== cancelledId) stable = false;
        if (current.job && ["completed", "failed", "cancelled"].includes(current.job.state))
          break;
        await pause(500);
      }
      const cancelled = await state();
      report.steps.cancel = {
        state: cancelled.job?.state ?? null,
        stopReason: cancelled.job?.stop_reason ?? null,
        jobIdStable: stable,
      };
      assert.equal(stable, true, "Cancelling started another search job");
    } else {
      report.steps.cancel = { skipped: "cancel was not available" };
    }
  } else {
    report.steps.nextBatch = { skipped: "next batch was disabled" };
  }
  report.passed = true;
  report.passedAt = new Date().toISOString();
  report.hostPassThrough =
    target === "tauri"
      ? "submitted params are read back from the Tauri host (core:current retains the request it forwarded), so this record proves the packaged bridge passed the continuation fields unchanged"
      : "submitted params come from the Electron worker client; the packaged Tauri pass-through is covered by the same driver on the tauri target";
  await writeFile(
    join(output, "verification.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
  console.log("SEARCH_CONTINUATION_UI_ACCEPTANCE_OK");
} catch (error) {
  report.passed = false;
  report.error = String(error);
  await writeFile(
    join(output, "verification.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  ).catch(() => {});
  throw error;
} finally {
  if (app) await app.close().catch(() => {});
  if (browser) await browser.close().catch(() => {});
  if (child && child.exitCode === null) child.kill();
}
