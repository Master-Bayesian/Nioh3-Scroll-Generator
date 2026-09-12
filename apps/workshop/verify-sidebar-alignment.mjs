import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

async function measureSidebar(page) {
  return page.locator('.shell > .nav').evaluate(nav => {
    const controls = [
      ['toggle', nav.querySelector('.nav-toggle')],
      ...Array.from(nav.querySelectorAll('nav > button')).map((button, index) => [
        `navigation-${index}`,
        button.querySelector('.nav-icon'),
      ]),
      ['language', nav.querySelector('.language-button > svg')],
      ['settings', nav.querySelector('.settings > .nav-icon')],
    ];
    return Object.fromEntries(controls.map(([name, element]) => {
      if (!element) throw new Error(`Missing sidebar icon: ${name}`);
      const rect = element.getBoundingClientRect();
      return [name, {
        centerY: rect.top + rect.height / 2,
        height: rect.height,
        width: rect.width,
        visible: getComputedStyle(element).visibility !== 'hidden',
        parent: { top: element.parentElement.getBoundingClientRect().top, height: element.parentElement.getBoundingClientRect().height },
      }];
    }));
  });
}

async function setCollapsed(page, collapsed) {
  const toggle = page.locator('.shell > .nav > .nav-toggle');
  if ((await toggle.getAttribute('aria-expanded')) === String(collapsed)) {
    await toggle.click();
  }
  await page.waitForFunction(expected => (
    document.querySelector('.shell').classList.contains('nav-collapsed') === expected
  ), collapsed);
}

/** Assert player-visible row positions without using CSS implementation details. */
export async function verifySidebarAlignment(page, { outputDirectory } = {}) {
  if (outputDirectory) await mkdir(outputDirectory, { recursive: true });
  const screenshot = async name => {
    if (outputDirectory) {
      await page.screenshot({ path: join(outputDirectory, `sidebar-${name}.png`) });
    }
  };
  await setCollapsed(page, false);
  const expanded = await measureSidebar(page);
  await setCollapsed(page, true);
  const collapsed = await measureSidebar(page);
  await setCollapsed(page, false);
  const restored = await measureSidebar(page);
  if (outputDirectory) await writeFile(join(outputDirectory, 'sidebar-geometry.json'), JSON.stringify({ expanded, collapsed, restored }, null, 2));
  assert.equal(Object.keys(expanded).length, 8, 'Every sidebar control must be measured');
  for (const [name, initial] of Object.entries(expanded)) {
    for (const [mode, values] of Object.entries({ expanded, collapsed, restored })) {
      const control = values[name];
      assert(control.visible && control.width > 0 && control.height > 0,
        `${name} must remain visible in the ${mode} sidebar`);
      assert(Math.abs(control.centerY - initial.centerY) <= 0.5,
        `${name} moved vertically by ${control.centerY - initial.centerY}px when ${mode}`);
    }
  }
  // Element screenshots can resize a fractional-DPI viewport by one pixel.
  // Measure every state first, then capture the window without changing its size.
  await screenshot('expanded');
  await setCollapsed(page, true);
  await screenshot('collapsed');
  await setCollapsed(page, false);
  return { expanded, collapsed, restored };
}
