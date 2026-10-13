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

/** Open each catalog section's help from its tab; it must fit, stay on top and close. */
export async function verifySectionHelp(page, { outputDirectory } = {}) {
  if (outputDirectory) await mkdir(outputDirectory, { recursive: true });
  const tabs = page.locator('.catalog-tab');
  assert.equal(await tabs.count(), 5, 'All five filtering sections must expose help');
  const results = [];
  for (let index = 0; index < await tabs.count(); index++) {
    const tab = tabs.nth(index);
    await tab.click();
    const help = page.locator('.catalog-column .section-help-trigger:visible').first();
    await help.click();
    await page.locator('.section-help-popover').waitFor({ state: 'visible' });
    const geometry = await popupGeometry(page);
    assert(geometry.textLength > 20, 'Help must include its complete explanation');
    assert(geometry.unobscured, 'Nothing may cover the floating help');
    assert(geometry.left >= 0 && geometry.top >= 0 &&
      geometry.right <= geometry.viewportWidth && geometry.bottom <= geometry.viewportHeight,
    'The complete help popover must fit within the application viewport');
    if (outputDirectory) await page.screenshot({ path: join(outputDirectory, `section-help-${index}.png`) });
    await page.keyboard.press('Escape');
    await page.locator('.section-help-popover').waitFor({ state: 'detached' });
    results.push({ section: (await tab.innerText()).trim(), ...geometry });
  }
  return results;
}
