/** Draft, not yet run: geometry acceptance for rendered cart/history cards; no game or clipboard actions. */
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

/**
 * Catalog seeds are hints only: regenerate each through the ordinary backend preview.
 * The caller must provide an isolated app profile with the search page visible.
 */
export async function addKnownEnemyVariantsToCart(page, {
  seeds = [12008, 12011, 12019], addToFavorites = false,
} = {}) {
  assert.equal(await page.locator('dialog[open]').count(), 0, 'Close dialogs before populating collection samples');
  const results = [];
  for (const seed of seeds) {
    await page.getByRole('textbox', { name: '已知绘卷ID', exact: true }).fill(String(seed));
    await page.getByRole('button', { name: '查看', exact: true }).click();
    await page.waitForFunction(value => document.querySelector('.result-detail .scroll footer strong')?.textContent === value,
      String(seed), { timeout: 30000 });
    const enemyCount = await page.locator('.result-detail .enemy-lines > span').count();
    const add = page.locator('.result-cart-actions .cart-toggle');
    if (await add.getAttribute('aria-pressed') !== 'true') await add.click();
    await page.waitForFunction(() => document.querySelector('.result-cart-actions .cart-toggle')?.getAttribute('aria-pressed') === 'true');
    if (addToFavorites) {
      const favorite = page.locator('.result-tools .favorite-button');
      if (await favorite.getAttribute('aria-pressed') !== 'true') await favorite.click();
      await page.waitForFunction(() => document.querySelector('.result-tools .favorite-button')?.getAttribute('aria-pressed') === 'true');
    }
    results.push({ seed, enemyCount, source: 'Actual backend known-ID preview; catalog supplied seed hints only' });
  }
  assert(new Set(results.map(value => value.enemyCount)).size >= 2,
    `Known seeds must actually render different enemy counts: ${JSON.stringify(results)}`);
  return results;
}

async function measurePreview(page) {
  const preview = page.locator('.result-detail > .scroll');
  if (!await preview.count()) return null;
  return preview.evaluate(card => {
    const rows = selector => [...card.querySelectorAll(selector)].map(element => {
      const box = element.getBoundingClientRect(), style = getComputedStyle(element);
      const name = element.querySelector('div > span, :scope > span:not(.slot-mark)');
      const value = element.querySelector('strong');
      const nameBox = name?.getBoundingClientRect(), valueBox = value?.getBoundingClientRect();
      const fontSize = parseFloat(style.fontSize);
      const lineHeight = parseFloat(style.lineHeight) || fontSize * 1.2;
      return { top: box.top, bottom: box.bottom, height: box.height, fontSize, lineHeight,
        empty: element.classList.contains('empty-line'),
        nameRight: nameBox?.right ?? null, valueLeft: valueBox?.left ?? null };
    });
    const rectangle = card.getBoundingClientRect(), footer = card.querySelector('footer').getBoundingClientRect();
    const header = card.querySelector(':scope > header'), title = header.querySelector('.scroll-title');
    const titleBox = title.getBoundingClientRect(), markBox = header.querySelector('.rarity-mark').getBoundingClientRect();
    return { height: rectangle.height, bottom: rectangle.bottom, footerBottom: footer.bottom,
      title: { text: title.textContent.trim(), accessibleText: title.getAttribute('title'),
        top: titleBox.top, bottom: titleBox.bottom, right: titleBox.right, markLeft: markBox.left,
        clientHeight: title.clientHeight, scrollHeight: title.scrollHeight },
      effects: rows('.effect-line'), rules: rows('.scroll-rule') };
  });
}

/**
 * Call with an already-open cart or history dialog. Does not add/remove/commit items.
 * Every card is measured before scrolling so differently populated rows are comparable.
 */
export async function verifyCollectionLayout(page, {
  container = '.cart-review', outputDirectory, label = 'collection', requireVariedEnemyCounts = true,
  minimumCards = 2, verifyPreview = true,
} = {}) {
  if (outputDirectory) await mkdir(outputDirectory, { recursive: true });
  const collection = page.locator(container);
  await collection.waitFor({ state: 'visible' });
  const cards = collection.locator('.compare-grid > div > .scroll');
  assert(await cards.count() >= minimumCards, `Collection requires at least ${minimumCards} real cards`);
  const geometry = await cards.evaluateAll(elements => elements.map(card => {
    const rectangle = element => {
      const r = element.getBoundingClientRect();
      return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, height: r.height, width: r.width };
    };
    const item = card.parentElement;
    const actionWrapper = item.querySelector('.collection-actions');
    const buttons = actionWrapper ? [...actionWrapper.querySelectorAll('button')] : [...item.querySelectorAll(':scope > button')];
    const enemyList = card.querySelector('.enemy-lines');
    const footer = card.querySelector('footer');
    const header = card.querySelector(':scope > header'), title = header.querySelector('.scroll-title');
    const titleBox = title.getBoundingClientRect(), headerBox = header.getBoundingClientRect();
    const markBox = header.querySelector('.rarity-mark').getBoundingClientRect();
    const enemyLabels = [...enemyList.querySelectorAll(':scope > span')].map(label => {
      const style = getComputedStyle(label);
      return { text: label.textContent.trim(), display: style.display, whiteSpace: style.whiteSpace,
        overflow: style.overflow, textOverflow: style.textOverflow,
        scrollWidth: label.scrollWidth, clientWidth: label.clientWidth };
    });
    const previousEnemyScroll = enemyList.scrollTop;
    enemyList.scrollTop = enemyList.scrollHeight;
    const lastEnemy = enemyList.lastElementChild?.getBoundingClientRect();
    const enemyBoxAtEnd = enemyList.getBoundingClientRect();
    const lastEnemyReachable = !lastEnemy || (lastEnemy.bottom <= enemyBoxAtEnd.bottom + 1 && lastEnemy.top >= enemyBoxAtEnd.top - 1);
    enemyList.scrollTop = previousEnemyScroll;
    return { seed: card.querySelector('footer strong')?.textContent, card: rectangle(card), item: rectangle(item),
      footer: rectangle(footer), enemyCount: enemyList.querySelectorAll(':scope > span').length,
      enemyList: { ...rectangle(enemyList), scrollHeight: enemyList.scrollHeight, clientHeight: enemyList.clientHeight,
        overflowY: getComputedStyle(enemyList).overflowY, labels: enemyLabels, lastEnemyReachable },
      title: { text: title.textContent.trim(), accessibleText: title.getAttribute('title'),
        top: titleBox.top, bottom: titleBox.bottom, right: titleBox.right, headerBottom: headerBox.bottom,
        markLeft: markBox.left, clientHeight: title.clientHeight, scrollHeight: title.scrollHeight },
      actions: buttons.map(button => ({ label: button.getAttribute('aria-label') || button.textContent.trim(),
        ...rectangle(button) })) };
  }));
  const preview = verifyPreview ? await measurePreview(page) : null;
  const results = { container, geometry, preview, scope: 'Rendered frontend geometry; no save writes, game calls, or synthetic enemy injection' };
  if (outputDirectory) await writeFile(join(outputDirectory, `${label}-geometry.json`), JSON.stringify(results, null, 2));
  const heights = geometry.map(value => value.card.height);
  assert(Math.max(...heights) - Math.min(...heights) <= 1,
    `Card heights must match regardless of enemy count: ${JSON.stringify(geometry.map(value => [value.seed, value.enemyCount, value.card.height]))}`);
  if (requireVariedEnemyCounts) assert(new Set(geometry.map(value => value.enemyCount)).size >= 2,
    'The acceptance must contain cards with genuinely different enemy counts');
  for (const value of geometry) {
    assert(value.footer.bottom <= value.card.bottom + 1 && value.footer.top >= value.card.top,
      `The complete ID/copy footer fits inside card ${value.seed}`);
    assert(value.actions.length > 0, `Card ${value.seed} exposes its collection actions`);
    const centers = value.actions.map(button => button.top + button.height / 2);
    assert(Math.max(...centers) - Math.min(...centers) <= 1,
      `Favorite and collection actions share a baseline on card ${value.seed}`);
    for (const button of value.actions) assert(button.height >= 28 && button.width >= 28,
      `Collection actions remain usable targets on card ${value.seed}: ${button.label}`);
    if (value.enemyList.scrollHeight > value.enemyList.clientHeight + 1) {
      assert(['auto', 'scroll'].includes(value.enemyList.overflowY),
        `Long enemy lists remain scrollable instead of clipping card ${value.seed}`);
    }
    assert(value.enemyList.lastEnemyReachable, `Every enemy label is reachable in card ${value.seed}`);
    for (const enemy of value.enemyList.labels) {
      assert(enemy.text && enemy.display !== 'none', `Card ${value.seed} retains every enemy label`);
      assert(enemy.whiteSpace !== 'nowrap' && enemy.textOverflow !== 'ellipsis' && enemy.scrollWidth <= enemy.clientWidth + 1,
        `Enemy label remains complete without ellipsis in card ${value.seed}: ${JSON.stringify(enemy)}`);
    }
    assert(value.title.bottom <= value.title.headerBottom + 1 && value.title.right <= value.title.markLeft + 1,
      `Localized title stays inside the header without overlapping card metadata ${value.seed}`);
    assert(value.title.scrollHeight <= value.title.clientHeight + 1 && value.title.accessibleText === value.title.text,
      `Clamped localized title preserves its complete accessible text in card ${value.seed}`);
  }
  // Compare cards that occupy the same grid row, including labels above each card.
  for (let index = 0; index < geometry.length; index++) {
    for (const other of geometry.slice(index + 1)) {
      const value = geometry[index];
      if (Math.abs(value.item.top - other.item.top) <= 1) {
        assert(Math.abs(value.footer.top - other.footer.top) <= 1,
          `ID footers align across one collection row: ${value.seed}, ${other.seed}`);
        assert(Math.abs(value.actions[0].top + value.actions[0].height / 2 -
          other.actions[0].top - other.actions[0].height / 2) <= 1,
        `Collection actions align across one grid row: ${value.seed}, ${other.seed}`);
      }
    }
  }
  if (preview) {
    assert(preview.footerBottom <= preview.bottom + 1, 'Main preview ID/copy footer remains inside its fixed card');
    assert(preview.title.bottom <= preview.title.clientHeight + preview.title.top + 1 &&
      preview.title.right <= preview.title.markLeft + 1 && preview.title.accessibleText === preview.title.text,
    'Main preview localized title remains contained and exposes its full text');
    for (const [kind, rows] of [['effects', preview.effects], ['rules', preview.rules]]) {
      for (const row of rows) {
        assert(row.height >= row.lineHeight - 1, `Preview ${kind} rows fit their text line without vertical clipping`);
        if (!row.empty && row.nameRight !== null && row.valueLeft !== null)
          assert(row.nameRight <= row.valueLeft + 1, `Preview ${kind} names and values do not overlap`);
      }
      for (let index = 1; index < rows.length; index++) assert(rows[index].top >= rows[index - 1].bottom - 1,
        `Preview ${kind} rows do not overlap each other`);
    }
  }

  const lastItem = cards.last().locator('..');
  const lastActions = lastItem.locator('.collection-actions button');
  const actionTargets = await lastActions.count() ? lastActions : lastItem.locator(':scope > button');
  const lastAction = actionTargets.last();
  await lastAction.scrollIntoViewIfNeeded();
  results.lastActions = await actionTargets.evaluateAll(buttons => buttons.map(button => {
    const r = button.getBoundingClientRect();
    const points = [[r.left + 2, r.top + 2], [r.right - 2, r.top + 2],
      [r.left + 2, r.bottom - 2], [r.right - 2, r.bottom - 2], [r.left + r.width / 2, r.top + r.height / 2]];
    return { label: button.getAttribute('aria-label') || button.textContent.trim(), top: r.top, bottom: r.bottom,
      left: r.left, right: r.right, viewportHeight: innerHeight, viewportWidth: innerWidth,
      unobscured: points.every(([x, y]) => document.elementsFromPoint(x, y).includes(button)) };
  }));
  results.lastAction = results.lastActions.at(-1);
  for (const action of results.lastActions) assert(action.unobscured && action.top >= 0 && action.bottom <= action.viewportHeight + 1,
    `The complete last collection action row is reachable after scrolling: ${action.label}`);
  if (outputDirectory) {
    await writeFile(join(outputDirectory, `${label}-geometry.json`), JSON.stringify(results, null, 2));
    await page.screenshot({ path: join(outputDirectory, `${label}-last-action.png`) });
  }
  return results;
}
