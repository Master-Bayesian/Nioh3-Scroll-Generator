import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';

async function popupGeometry(page) {
  return page.locator('.section-help-popover').evaluate(popup => {
    const rect = popup.getBoundingClientRect();
    const points = [
      [rect.left + 6, rect.top + 6],
      [rect.right - 6, rect.top + 6],
      [rect.left + 6, rect.bottom - 6],
      [rect.right - 6, rect.bottom - 6],
      [rect.left + rect.width / 2, rect.top + rect.height / 2],
    ];
    return {
      textLength: popup.textContent.trim().length,
      left: rect.left,
      top: rect.top,
      right: rect.right,
      bottom: rect.bottom,
      viewportWidth: window.innerWidth,
      viewportHeight: window.innerHeight,
      unobscured: points.every(([x, y]) => document.elementsFromPoint(x, y).includes(popup)),
    };
  });
}

/** Exercise both collapsed help and dismissal after an accordion closes. */
export async function verifySectionHelp(page, { outputDirectory } = {}) {
  if (outputDirectory) await mkdir(outputDirectory, { recursive: true });
  const panels = page.locator('.catalog-column > .module');
  assert.equal(await panels.count(), 5, 'All five filtering sections must expose help');
  const results = [];
  for (let index = 0; index < await panels.count(); index++) {
    const panel = panels.nth(index);
    const toggle = panel.locator('.panel-toggle');
    const help = panel.locator('.section-help-trigger');
    if (await toggle.getAttribute('aria-expanded') === 'true') await toggle.click();
    await help.click();
    await page.locator('.section-help-popover').waitFor({ state: 'visible' });
    const geometry = await popupGeometry(page);
    assert(geometry.textLength > 20, 'Help must include its complete explanation');
    assert(geometry.unobscured, 'Collapsed section borders must not clip floating help');
    assert(geometry.left >= 0 && geometry.top >= 0 &&
      geometry.right <= geometry.viewportWidth && geometry.bottom <= geometry.viewportHeight,
    'The complete help popover must fit within the application viewport');
    assert.equal(await toggle.getAttribute('aria-expanded'), 'false',
      'Opening help must not expand its filter section');
    if (outputDirectory) await page.screenshot({ path: join(outputDirectory, `section-help-${index}.png`) });
    await toggle.click();
    await page.locator('.section-help-popover').waitFor({ state: 'detached' });
    await help.click();
    await page.locator('.section-help-popover').waitFor({ state: 'visible' });
    await toggle.click();
    await page.locator('.section-help-popover').waitFor({ state: 'detached' });
    assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
    await help.click();
    await page.keyboard.press('Escape');
    await page.locator('.section-help-popover').waitFor({ state: 'detached' });
    results.push({ section: await toggle.getAttribute('aria-label'), ...geometry });
  }
  return results;
}
