/** Native WebView2 regression acceptance for Japanese text, favorites and update prompts. */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { mkdir, mkdtemp, realpath, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";

const root = await realpath(await mkdtemp(join(tmpdir(), "nioh3-v074-hotfix-")));
const output = resolve(
  process.env.NIOH3_UI_OUTPUT || "deliverables/v074-update-acceptance-20260914",
);
await mkdir(output, { recursive: true });
const python = process.env.NIOH3_PYTHON;
assert(python, "NIOH3_PYTHON must name the prepared project environment");

const server = createServer();
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const port = server.address().port;
await new Promise((done) => server.close(done));
const executable = resolve(
  process.env.NIOH3_PORTABLE_EXE ||
    "apps/tauri/src-tauri/target/debug/nioh3-studio.exe",
);
const child = spawn(executable, ["--user-data-dir", join(root, "profile")], {
  windowsHide: true,
  stdio: ["ignore", "pipe", "pipe"],
  env: {
    ...process.env,
    NIOH3_PYTHON: python,
    NIOH3_REVIEW_UI: "1",
    NIOH3_TAURI_TEST_ROOT: join(root, "profile"),
    NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
    NIOH3_STATE_ROOT: join(root, "state"),
    LOCALAPPDATA: join(root, "local"),
  },
});
let stderr = "";
child.stderr.on("data", (data) => (stderr = (stderr + data).slice(-32000)));
let browser;

async function waitForDebugEndpoint() {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (child.exitCode !== null)
      throw Error(`App exited ${child.exitCode}: ${stderr}`);
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return;
    } catch {}
    await new Promise((done) => setTimeout(done, 250));
  }
  throw Error("WebView2 debug endpoint did not start");
}

try {
  console.log("HOTFIX_ACCEPTANCE_STAGE app-start");
  await waitForDebugEndpoint();
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0];
  const page = context.pages()[0] || (await context.waitForEvent("page"));
  await page
    .getByText("后端已连接，请选择筛选条件。", { exact: true })
    .waitFor({ timeout: 30000 });

  console.log("HOTFIX_ACCEPTANCE_STAGE japanese-favorite");
  await page.getByRole("textbox", { name: "已知绘卷ID", exact: true }).fill("76634363");
  await page.getByRole("button", { name: "查看", exact: true }).click();
  await page.locator(".result-detail .scroll").waitFor({ timeout: 30000 });
  await page.locator(".language-button").click();
  await page
    .locator(".side-popup")
    .getByRole("button", { name: "日本語", exact: true })
    .click();
  const card = page.locator(".result-detail .scroll");
  assert.doesNotMatch(await card.innerText(), /RUBY|\^(?:20|21|FE|FF)~/);
  assert.match(await card.innerText(), /マガツヒの恩寵/);
  const star = page.locator(".result-tools .favorite-button");
  await star.waitFor();
  assert.equal(await star.locator("svg path").getAttribute("fill"), "none");
  await star.click();
  await page.waitForFunction(
    () =>
      document
        .querySelector(".result-tools .favorite-button")
        ?.getAttribute("aria-pressed") === "true",
  );
  assert.equal(await star.locator("svg path").getAttribute("fill"), "currentColor");
  await star.click();

  const geometry = [];
  for (const zoom of [1, 1.25, 1.5]) {
    await page.evaluate((value) => {
      document.documentElement.style.zoom = String(value);
    }, zoom);
    const measured = await card.locator(".effect-line").evaluateAll((lines) =>
      lines.map((element) => {
        const row = element.getBoundingClientRect();
        const text = element.querySelector("div > span")?.getBoundingClientRect();
        return {
          top: row.top,
          bottom: row.bottom,
          textTop: text?.top,
          textBottom: text?.bottom,
        };
      }),
    );
    for (const line of measured) {
      if (line.textTop !== undefined) {
        assert.ok(
          line.textTop >= line.top - 2 && line.textBottom <= line.bottom + 2,
          "Japanese name stays in its row",
        );
      }
    }
    geometry.push({ zoom, measured });
    await page.screenshot({ path: join(output, `japanese-${zoom}.png`) });
  }

  console.log("HOTFIX_ACCEPTANCE_STAGE startup-update");
  await page.addInitScript(() => {
    let phase = "idle";
    let updateChecks = 0;
    const install = (review) => {
      review.update = async ({ action }) => {
        if (action === "check") {
          updateChecks += 1;
          phase = "available";
        }
        return {
          phase,
          version: "0.8.0",
          notes: "Local acceptance response",
          canApply: true,
        };
      };
      Object.defineProperty(window, "review", {
        configurable: true,
        writable: true,
        value: review,
      });
    };
    Object.defineProperty(window, "__nioh3UpdateChecks", {
      configurable: true,
      get: () => updateChecks,
    });
    Object.defineProperty(window, "review", {
      configurable: true,
      set: install,
    });
  });
  await page.evaluate(() => localStorage.setItem("nioh3-ui-locale", "zh-CN"));
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.locator(".update-panel").waitFor({ timeout: 15000 });
  assert.equal(await page.evaluate(() => window.__nioh3UpdateChecks), 1);
  assert.match(await page.locator(".update-panel").innerText(), /是否现在下载并安装/);
  await page.screenshot({ path: join(output, "startup-update-prompt.png") });
  await page
    .locator(".update-panel")
    .getByRole("button", { name: "稍后", exact: true })
    .click();
  assert.equal(await page.locator(".update-panel").isVisible(), false);

  console.log("HOTFIX_ACCEPTANCE_STAGE manual-update");
  await page.locator(".settings").click();
  await page
    .locator(".side-popup")
    .getByRole("button", { name: "检查更新", exact: true })
    .click();
  await page.locator(".update-panel").waitFor();
  await page
    .locator(".update-panel")
    .getByRole("button", { name: "检查更新", exact: true })
    .click();
  assert.equal(await page.evaluate(() => window.__nioh3UpdateChecks), 2);
  await page.screenshot({ path: join(output, "settings-manual-update.png") });

  await writeFile(
    join(output, "verification.json"),
    `${JSON.stringify(
      {
        seed: 76634363,
        geometry,
        automaticCheck: true,
        automaticPrompt: true,
        manualCheck: true,
        starStates: ["outline", "filled"],
        gameWrites: 0,
        transport: "local page-injected response",
      },
      null,
      2,
    )}\n`,
  );
  console.log("V074_JAPANESE_STARS_UPDATE_PROMPT_OK");
} finally {
  if (browser) await browser.close().catch(() => {});
  if (child.exitCode === null) child.kill();
}
