async (page) => {
  const check = (value, message) => { if (!value) throw new Error(message); };
  const origin = new URL(page.url()).origin;
  const base = `${origin}/color-scheme-review/`;
  const key = 'grapharch:color-scheme';
  const results = [];
  const contrast = (a, b) => {
    const luminance = color => {
      const values = color.match(/[\d.]+/g).slice(0, 3).map(value => {
        const x = Number(value) / 255;
        return x <= .04045 ? x / 12.92 : ((x + .055) / 1.055) ** 2.4;
      });
      return values[0] * .2126 + values[1] * .7152 + values[2] * .0722;
    };
    const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
    return (values[0] + .05) / (values[1] + .05);
  };
  for (const style of ['ekgf', 'ekgf-project']) {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.goto(`${base}${style}/site/`);
    await page.evaluate(key => localStorage.removeItem(key), key);
    await page.reload();
    const scheme = () => page.evaluate(() => document.documentElement.dataset.colorScheme);
    check(await scheme() === 'dark', `${style}: initial OS preference`);
    await page.emulateMedia({ colorScheme: 'light' });
    await page.waitForFunction(() => document.documentElement.dataset.colorScheme === 'light');
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.waitForFunction(() => document.documentElement.dataset.colorScheme === 'dark');
    const toggle = page.getByRole('button', { name: 'Dark theme', exact: true });
    check(await toggle.getAttribute('aria-pressed') === 'true', 'Accessible initial state');
    await toggle.focus();
    await page.keyboard.press('Enter');
    check(await scheme() === 'light', 'Keyboard toggles to light');
    check(await page.evaluate(key => localStorage.getItem(key), key) === 'light', 'Explicit choice persisted');
    await page.emulateMedia({ colorScheme: 'light' });
    await page.emulateMedia({ colorScheme: 'dark' });
    check(await scheme() === 'light', 'OS cannot override explicit choice');
    await page.reload();
    check(await scheme() === 'light', 'Reload preserves choice');
    for (const route of ['guide/nested/', 'provenance.html', 'quality-issues.html']) {
      await page.goto(`${base}${style}/site/${route}`);
      check(await scheme() === 'light', `${style}/${route}: shared explicit preference`);
      check(await page.getByRole('button', { name: 'Dark theme', exact: true }).count() === 1, 'One shared control');
    }
    await page.getByRole('button', { name: 'Dark theme', exact: true }).click();
    await page.goto(`${base}${style}/site/`);
    check(await scheme() === 'dark', 'Explicit dark follows navigation');
    const geometry = await page.evaluate(() => {
      const get = selector => {
        const node = document.querySelector(selector);
        const style = getComputedStyle(node);
        return { color: style.color, background: style.backgroundColor };
      };
      return { body: get('body'), note: get('.markdown-alert-note'), warning: get('.markdown-alert-warning'), footer: get('.ekgf-footer'), code: get('pre'), button: document.querySelector('.color-scheme-toggle').getBoundingClientRect().width, overflow: document.documentElement.scrollWidth > innerWidth };
    });
    check(contrast(geometry.body.color, geometry.body.background) >= 7, 'Dark body contrast');
    check(contrast(geometry.note.color, geometry.note.background) >= 4.5, 'Dark note contrast');
    check(contrast(geometry.warning.color, geometry.warning.background) >= 4.5, 'Dark warning contrast');
    check(contrast(geometry.footer.color, geometry.footer.background) >= 4.5, 'Footer contrast');
    check(geometry.button >= 44 && !geometry.overflow, 'Desktop target size and overflow');
    const colors = await page.locator('pre span').evaluateAll(nodes => nodes.map(node => getComputedStyle(node).color));
    for (const color of colors) check(contrast(color, geometry.code.background) >= 4.5, `Code contrast: ${color}`);
    await page.screenshot({ path: `output/color-scheme-${style}-desktop.png` });
    await page.setViewportSize({ width: 390, height: 900 });
    check(!await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), 'Mobile overflow');
    await toggle.focus();
    await page.keyboard.press('Space');
    check(await scheme() === 'light', 'Mobile keyboard toggles');
    check(!await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), 'Light mobile overflow');
    await page.keyboard.press('Space');
    check(await scheme() === 'dark', 'Mobile keyboard toggles back');
    await page.screenshot({ path: `output/color-scheme-${style}-mobile.png` });
    results.push({ style, ...geometry });
  }
  await page.goto(`${base}reader/site/`);
  check(await page.locator('.color-scheme-toggle').count() === 0, 'Neutral theme remains unchanged');
  check(!await page.evaluate(() => document.documentElement.hasAttribute('data-color-scheme')), 'No community preference leak into neutral publisher');
  return results;
}
