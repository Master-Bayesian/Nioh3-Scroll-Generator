/** Native WebView2 acceptance for enemy-state filtering and preview presentation. */
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, realpath, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { createServer } from "node:net";
import assert from "node:assert/strict";

const root = await realpath(await mkdtemp(join(tmpdir(), "nioh3-enemy-ui-")));
const output = resolve(
  process.env.NIOH3_UI_OUTPUT || "deliverables/enemy-states-ui-20260914",
);
await mkdir(output, { recursive: true });
const python = process.env.NIOH3_PYTHON;
assert(python, "NIOH3_PYTHON must name the prepared project environment");

const server = createServer();
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const port = server.address().port;
await new Promise((done) => server.close(done));
const executable = resolve(
  process.env.NIOH3_TAURI_EXE ||
    "apps/tauri/src-tauri/target/debug/nioh3-studio.exe",
);
const child = spawn(executable, ["--user-data-dir", join(root, "profile")], {
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

async function preview(page, seed) {
  await page.getByRole("textbox", { name: "已知绘卷ID", exact: true }).fill(String(seed));
  await page.getByRole("button", { name: "查看", exact: true }).click();
  await page
    .locator(`.result-detail .scroll[aria-label="绘卷 ${seed}"]`)
    .waitFor({ timeout: 30000 });
}

async function metrics(page) {
  return page.evaluate(() => {
    const card = document.querySelector(".result-detail .scroll");
    const enemySection = card?.querySelectorAll("section")[1];
    const heroRows = [...(card?.querySelectorAll(".scroll-hero p") || [])];
    return {
      viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
      documentFits: document.documentElement.scrollWidth <= innerWidth,
      cardFits: card ? card.scrollWidth <= card.clientWidth + 1 : false,
      cardWidth: card
        ? { client: card.clientWidth, scroll: card.scrollWidth }
        : null,
      overflowers: card
        ? [...card.querySelectorAll("*")]
            .filter((element) => {
              const node = element;
              const cardRect = card.getBoundingClientRect();
              const rect = node.getBoundingClientRect();
              return (
                node.scrollWidth > node.clientWidth + 1 ||
                rect.right > cardRect.right + 1 ||
                rect.left < cardRect.left - 1
              );
            })
            .slice(0, 12)
            .map((element) => ({
              tag: element.tagName,
              className: element.className,
              text: element.textContent?.trim().slice(0, 80),
              client: element.clientWidth,
              scroll: element.scrollWidth,
              left: Math.round(element.getBoundingClientRect().left),
              right: Math.round(element.getBoundingClientRect().right),
            }))
        : [],
      enemySectionFits: enemySection
        ? enemySection.scrollHeight <= enemySection.clientHeight + 1
        : false,
      hero: heroRows.map((row) => {
        const value = row.querySelector("strong");
        return {
          labelFont: getComputedStyle(row).fontSize,
          valueFont: value ? getComputedStyle(value).fontSize : "",
          labelLine: getComputedStyle(row).lineHeight,
          valueLine: value ? getComputedStyle(value).lineHeight : "",
        };
      }),
    };
  });
}

async function possessedBadgePlacement(page) {
  return page.locator(".enemy-occurrence:has(.enemy-state-marker)").first().evaluate((element) => {
    const label = element.querySelector(":scope > span");
    const badge = label?.querySelector(".enemy-state-marker");
    if (!label || !badge)
      throw new Error("Possessed occurrence markup is incomplete");
    const nameRange = document.createRange();
    nameRange.setStart(label, 0);
    nameRange.setEndBefore(badge);
    const nameRect = nameRange.getBoundingClientRect();
    const rowRect = element.getBoundingClientRect();
    const badgeRect = badge.getBoundingClientRect();
    return {
      gap: badgeRect.left - nameRect.right,
      centerDelta: Math.abs(
        (badgeRect.top + badgeRect.bottom) / 2 -
          (nameRect.top + nameRect.bottom) / 2,
      ),
      rowCenterDelta: Math.abs(
        (nameRect.top + nameRect.bottom) / 2 - (rowRect.top + rowRect.bottom) / 2,
      ),
      rowAlignItems: getComputedStyle(element).alignItems,
      labelAlignItems: getComputedStyle(label).alignItems,
      verticalAlign: getComputedStyle(badge).verticalAlign,
      text: badge.textContent?.trim(),
    };
  });
}

async function expandedShellPlacement(page) {
  return page.locator(".shell:not(.nav-collapsed)").evaluate((shell) => {
    const nav = shell.querySelector(":scope > .nav");
    const main = shell.querySelector(".search-page:not([hidden]) > main");
    if (!nav || !main) throw new Error("Expanded application shell is incomplete");
    const navRect = nav.getBoundingClientRect();
    const mainRect = main.getBoundingClientRect();
    return {
      columns: getComputedStyle(shell).gridTemplateColumns,
      navWidth: navRect.width,
      navRight: navRect.right,
      mainLeft: mainRect.left,
    };
  });
}

async function favoritesGeometry(page) {
  const cards = page.locator(".favorites-review .favorite-card");
  return cards.evaluateAll((items) =>
    items.map((item) => {
      const card = item.querySelector(":scope > .scroll");
      const actions = item.querySelector(":scope > .collection-actions");
      const cardRect = card.getBoundingClientRect();
      const actionButtons = [...actions.querySelectorAll("button")];
      const centers = actionButtons.map((button) => {
        const rect = button.getBoundingClientRect();
        return rect.top + rect.height / 2;
      });
      return {
        seed: card.querySelector("footer strong")?.textContent,
        cardHeight: cardRect.height,
        cardFits:
          card.scrollWidth <= card.clientWidth + 1 &&
          card.scrollHeight <= card.clientHeight + 1,
        actionCenterSpread: Math.max(...centers) - Math.min(...centers),
        itemWidth: item.getBoundingClientRect().width,
      };
    }),
  );
}

try {
  await waitForDebugEndpoint();
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  let page;
  for (let attempt = 0; attempt < 150; attempt += 1) {
    page = browser.contexts()[0]?.pages()[0];
    if (page) break;
    await new Promise((done) => setTimeout(done, 200));
  }
  assert(page, "WebView2 opened without a page target");
  await page
    .getByText("请选择筛选条件。", { exact: true })
    .waitFor({ timeout: 45000 });
  assert.equal(await page.locator(".intro").count(), 0);
  assert.equal(
    await page.getByText("装备在身上：筛选词条与恩宠", { exact: true }).count(),
    1,
  );
  assert.equal(
    await page
      .getByText("刷副本：筛选敌人、规则、地形与挑战次数", { exact: true })
      .count(),
    1,
  );
  assert.equal(
    await page.getByRole("button", { name: "使用说明", exact: true }).count(),
    1,
  );
  assert(
    (await page.locator(".selection").evaluate((element) => element.getBoundingClientRect().height)) >=
      220,
  );
  await page.getByRole("button", { name: "设置", exact: true }).click();
  assert.equal(await page.getByText("界面字号", { exact: true }).count(), 0);
  await page.locator(".popup-dismiss").click();
  await page.getByRole("button", { name: "敌人", exact: true }).click();

  assert.equal(await page.locator(".enemy-variant-choice").count(), 0);
  assert.equal(await page.getByRole("combobox", { name: /敌人状态|出场范围/ }).count(), 0);
  const enemyListFrame = await page.locator(".enemy-list-frame").evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      height: element.getBoundingClientRect().height,
      borderWidth: style.borderTopWidth,
      background: style.backgroundColor,
    };
  });
  assert(enemyListFrame.height >= 72);
  assert.notEqual(enemyListFrame.borderWidth, "0px");
  assert.notEqual(enemyListFrame.background, "rgba(0, 0, 0, 0)");

  await preview(page, 86872488);
  assert.equal(await page.locator(".enemy-occurrence").count(), 6);
  assert.equal(
    await page.locator('.enemy-state-marker[aria-label="地狱附身"]').count(),
    0,
  );
  const negativeMetrics = await metrics(page);
  console.log("NEGATIVE_METRICS", JSON.stringify(negativeMetrics));
  assert(negativeMetrics.documentFits && negativeMetrics.cardFits && negativeMetrics.enemySectionFits);
  assert.ok(
    negativeMetrics.hero.every(
      (row) => row.labelFont === row.valueFont && row.labelLine === row.valueLine,
    ),
  );
  await page.locator(".result-tools .favorite-button").click();
  await page.waitForFunction(
    () =>
      document
        .querySelector(".result-tools .favorite-button")
        ?.getAttribute("aria-pressed") === "true",
  );
  await page.screenshot({ path: join(output, "zh-possessed-negative.png") });

  await preview(page, 156062997);
  assert.equal(await page.locator(".enemy-occurrence").count(), 6);
  assert.equal(
    await page.locator('.enemy-state-marker[aria-label="地狱附身"]').count(),
    1,
  );
  const badgePlacement = await possessedBadgePlacement(page);
  assert.equal(badgePlacement.text, "附身");
  assert(badgePlacement.gap >= 0 && badgePlacement.gap <= 6);
  assert(badgePlacement.centerDelta <= 1);
  assert(badgePlacement.rowCenterDelta <= 1);
  assert.equal(badgePlacement.rowAlignItems, "center");
  assert.equal(badgePlacement.labelAlignItems, "center");
  const positiveMetrics = await metrics(page);
  console.log("POSITIVE_METRICS", JSON.stringify(positiveMetrics));
  assert(
    positiveMetrics.documentFits &&
      positiveMetrics.cardFits &&
      positiveMetrics.enemySectionFits,
  );
  const previewCardHeight = await page
    .locator(".result-detail > .scroll")
    .evaluate((element) => element.getBoundingClientRect().height);
  await page.locator(".result-tools .favorite-button").click();
  await page.waitForFunction(
    () =>
      document
        .querySelector(".result-tools .favorite-button")
        ?.getAttribute("aria-pressed") === "true",
  );
  await page.screenshot({ path: join(output, "zh-possessed-positive.png") });

  const restoredViewport = positiveMetrics.viewport;
  await page.evaluate(() => window.review.windowAction("maximize"));
  await page.waitForFunction(
    (previous) => innerWidth !== previous.width || innerHeight !== previous.height,
    restoredViewport,
  );
  const maximizedMetrics = await metrics(page);
  assert(
    maximizedMetrics.documentFits &&
      maximizedMetrics.cardFits &&
      maximizedMetrics.enemySectionFits,
  );
  await page.screenshot({ path: join(output, "zh-possessed-maximized.png") });
  await page.evaluate(() => window.review.windowAction("maximize"));
  await page.waitForFunction(
    (restored) =>
      Math.abs(innerWidth - restored.width) <= 2 &&
      Math.abs(innerHeight - restored.height) <= 2,
    restoredViewport,
  );

  const localeChecks = [
    {
      button: "English",
      possessed: "Crucible Wraith",
      marker: "Wraith",
      list: "Enemy list",
      eligible: "Crucible Wraith eligible",
      screenshot: "en-possessed.png",
      lang: "en-US",
    },
    {
      button: "日本語",
      possessed: "地獄憑き",
      marker: "憑き",
      list: "敵一覧",
      eligible: "地獄憑き対応",
      screenshot: "ja-possessed.png",
      lang: "ja-JP",
    },
  ];
  for (const locale of localeChecks) {
    await page.locator(".language-button").click();
    await page
      .locator(".side-popup")
      .getByRole("button", { name: locale.button, exact: true })
      .click();
    await page.waitForFunction((lang) => document.documentElement.lang === lang, locale.lang);
    await page.getByText(locale.list, { exact: true }).waitFor();
    assert.ok(await page.getByText(locale.eligible, { exact: true }).count());
    assert.equal(
      await page.locator(`.enemy-state-marker[aria-label="${locale.possessed}"]`).count(),
      1,
    );
    const localizedBadgePlacement = await possessedBadgePlacement(page);
    assert.equal(localizedBadgePlacement.text, locale.marker);
    assert(localizedBadgePlacement.gap >= 0 && localizedBadgePlacement.gap <= 6);
    assert(localizedBadgePlacement.centerDelta <= 1);
    assert(localizedBadgePlacement.rowCenterDelta <= 1);
    const shellPlacement = await expandedShellPlacement(page);
    assert.notEqual(shellPlacement.columns, "none");
    assert(shellPlacement.navWidth >= 80 && shellPlacement.navWidth <= 180);
    assert(shellPlacement.mainLeft >= shellPlacement.navRight - 1);
    const currentMetrics = await metrics(page);
    console.log(`${locale.lang.toUpperCase()}_METRICS`, JSON.stringify(currentMetrics));
    assert(
      currentMetrics.documentFits &&
        currentMetrics.cardFits &&
        currentMetrics.enemySectionFits,
    );
    await page.screenshot({ path: join(output, locale.screenshot) });
    await page.locator(".nav nav button:has(.star-icon)").click();
    const localizedFavorites = await favoritesGeometry(page);
    assert.equal(localizedFavorites.length, 2);
    assert(
      localizedFavorites.every(
        (value) =>
          value.cardFits &&
          Math.abs(value.cardHeight - previewCardHeight) <= 1 &&
          value.actionCenterSpread <= 1,
      ),
    );
    await page.screenshot({
      path: join(output, `${locale.lang}-favorites.png`),
    });
    await page.locator("dialog[open] > header > button").click();
  }

  await page.locator(".language-button").click();
  await page
    .locator(".side-popup")
    .getByRole("button", { name: "简体中文", exact: true })
    .click();
  await page.waitForFunction(() => document.documentElement.lang === "zh-CN");

  await page.getByRole("button", { name: "组合说明", exact: true }).click();
  const combinationDialog = page.getByRole("dialog");
  await combinationDialog.getByText("多个条件怎样计算？", { exact: true }).waitFor();
  assert.equal(
    await combinationDialog.getByText("怎样组合词条？", { exact: true }).count(),
    0,
  );
  await page.screenshot({ path: join(output, "zh-enemy-combination-help.png") });
  await combinationDialog.getByRole("button", { name: "关闭窗口" }).click();

  await page
    .getByRole("textbox", { name: "搜索全部敌人名称或 ID", exact: true })
    .fill("古笼火");
  const eligibleRow = page.locator(".catalog-row", { hasText: "古笼火" }).first();
  await eligibleRow.getByText("可附身", { exact: true }).waitFor();
  await eligibleRow.getByRole("button").click();
  const possessedSwitch = page.getByRole("switch", { name: "地狱附身", exact: true });
  assert.equal(await possessedSwitch.count(), 1);
  await page.locator(".toggle-switch.compact", { hasText: "地狱附身" }).click();
  assert.equal(await possessedSwitch.isChecked(), true);
  await page.mouse.move(1400, 800);
  await page.waitForFunction(
    () => !document.querySelector(".selection")?.classList.contains("selection-expanded"),
  );

  await page
    .getByRole("textbox", { name: "搜索全部敌人名称或 ID", exact: true })
    .fill("0x2EEE");
  const ineligibleLowRow = page.locator(".enemy-list .catalog-row").first();
  assert.equal(await page.locator(".enemy-list .catalog-row").count(), 1);
  assert.equal(await ineligibleLowRow.getByText("可附身", { exact: true }).count(), 0);
  await ineligibleLowRow.getByRole("button").click();
  assert.equal(await page.getByRole("switch", { name: "地狱附身", exact: true }).count(), 1);

  await page
    .getByRole("textbox", { name: "搜索全部敌人名称或 ID", exact: true })
    .fill("0x6E1BC");
  const nonLowRow = page.locator(".enemy-list .catalog-row").first();
  assert.equal(await page.locator(".enemy-list .catalog-row").count(), 1);
  assert.equal(await nonLowRow.getByText("可附身", { exact: true }).count(), 0);
  await nonLowRow.getByRole("button").click();
  assert.equal(await page.getByRole("switch", { name: "地狱附身", exact: true }).count(), 1);

  const filterSelection = {
    eligibleSwitchChecked: await possessedSwitch.isChecked(),
    totalPossessedSwitches: await page.getByRole("switch", { name: "地狱附身", exact: true }).count(),
    enemyStateSelects: await page.getByRole("combobox", { name: /敌人状态/ }).count(),
    expeditionControls: await page.locator(".enemy-variant-choice").count(),
  };
  const selectedEnemy = page.locator(".selected-body .enemy-chip").first();
  assert.equal(
    await selectedEnemy.evaluate(
      (element) => element.scrollWidth <= element.clientWidth + 1,
    ),
    true,
  );
  const selectedEnemyLayout = await selectedEnemy.evaluate((element) => {
    const parts = [
      element.querySelector(":scope > span:not(.drag-grip)"),
      element.querySelector(":scope > select"),
      element.querySelector(":scope > .toggle-switch"),
      element.querySelector(":scope > button"),
    ].filter(Boolean);
    const rectangles = parts.map((part) => part.getBoundingClientRect());
    const centers = rectangles.map((rect) => rect.top + rect.height / 2);
    const select = element.querySelector(":scope > select").getBoundingClientRect();
    return {
      rowHeight: element.getBoundingClientRect().height,
      centerSpread: Math.max(...centers) - Math.min(...centers),
      selectWidth: select.width,
    };
  });
  assert(selectedEnemyLayout.rowHeight <= 38);
  assert(selectedEnemyLayout.centerSpread <= 1);
  assert(selectedEnemyLayout.selectWidth <= 86);
  await page.screenshot({ path: join(output, "zh-possessed-filter.png") });

  await page.locator(".nav nav button:has(.star-icon)").click();
  const favoriteSearch = page.getByRole("textbox", {
    name: "搜索收藏夹中的绘卷 ID、词条、恩宠或敌人",
    exact: true,
  });
  await favoriteSearch.fill("86872488");
  assert.equal(await page.locator(".favorites-review .favorite-card").count(), 1);
  assert.equal(
    await page
      .locator(".favorites-review .favorite-card footer strong", {
        hasText: "86872488",
      })
      .count(),
    1,
  );
  await favoriteSearch.fill("does-not-exist");
  assert.equal(await page.locator(".favorites-review .favorite-card").count(), 0);
  assert.equal(
    await page.getByText("收藏夹中没有匹配的绘卷。", { exact: true }).count(),
    1,
  );
  await favoriteSearch.fill("");
  const favoriteLayout = await favoritesGeometry(page);
  assert.equal(favoriteLayout.length, 2);
  assert(
    favoriteLayout.every(
      (value) =>
        value.cardFits &&
        Math.abs(value.cardHeight - previewCardHeight) <= 1 &&
        value.actionCenterSpread <= 1,
    ),
  );
  assert(Math.max(...favoriteLayout.map((value) => value.cardHeight)) -
    Math.min(...favoriteLayout.map((value) => value.cardHeight)) <= 1);
  await page.screenshot({ path: join(output, "zh-favorites-search.png") });
  await page.locator("dialog[open] > header > button").click();

  const report = {
    passed: true,
    executable,
    controls: { expedition: "removed", curse: "removed" },
    negative: { seed: 86872488, occurrenceCount: 6, possessedCount: 0, metrics: negativeMetrics },
    positive: { seed: 156062997, occurrenceCount: 6, possessedCount: 1, metrics: positiveMetrics },
    possessedBadgePlacement: badgePlacement,
    maximized: maximizedMetrics,
    enemyListFrame,
    locales: ["zh-CN", "en-US", "ja-JP"],
    filterSelection,
    selectedEnemyLayout,
    favorites: {
      count: favoriteLayout.length,
      search: "local filter by scroll ID and visible card metadata",
      previewCardHeight,
      geometry: favoriteLayout,
    },
    possessedEligibility: {
      nativeEligibleLowPoolSwitch: "available",
      ineligibleLowPoolSwitch: "omitted",
      nonLowPoolSwitch: "omitted",
      workerGuard: "covered by protocol regression",
    },
    boundary:
      "Native WebView2 read-only preview acceptance; no game, save, or runtime write was performed.",
  };
  await writeFile(
    join(output, "verification.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
  console.log("ENEMY_STATE_UI_ACCEPTANCE_OK");
} finally {
  if (browser) await browser.close().catch(() => {});
  if (child.exitCode === null) child.kill();
}
