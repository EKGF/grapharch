async (page) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  const home = page.url();
  for (const width of [1920, 1440, 1000, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(home);
    await page.evaluate(() => document.fonts.ready);
    check(await page.locator('.project-hero').count() === 1, 'Project hero missing');
    check(await page.locator('.site-navigation').count() === 0, 'Documentation rail leaked into project layout');
    check(!await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), `Project overflows at ${width}`);
    if (width > 1150) {
      const community = await page.locator('.ekgf-identity span,.community-menu nav a').evaluateAll(nodes => nodes.map(node => {
        const marker = document.createElement('span');
        marker.style.cssText = 'display:inline-block;width:0;height:0;padding:0;margin:0';
        node.append(marker);
        const baseline = marker.getBoundingClientRect().top;
        marker.remove();
        return baseline;
      }));
      check(Math.max(...community) - Math.min(...community) < 1, `Community wordmark and links have different baselines at ${width}`);
    }
    const navigation = await page.locator('.project-navigation>ul').evaluate(list => {
      return [...list.children].map(item => {
        const anchor = item.querySelector('a');
        const marker = document.createElement('span');
        marker.style.cssText = 'display:inline-block;width:0;height:0;padding:0;margin:0';
        anchor.append(marker);
        const baseline = marker.getBoundingClientRect().top;
        marker.remove();
        return { baseline, top: item.getBoundingClientRect().top, margin: getComputedStyle(item).marginTop };
      });
    });
    check(navigation.every(item => item.margin === '0px'), 'Prose list spacing leaked into project navigation');
    const rows = new Map();
    for (const item of navigation) {
      const row = Math.round(item.top);
      const baselines = rows.get(row) || [];
      baselines.push(item.baseline);
      rows.set(row, baselines);
    }
    if (width >= 1440) check(rows.size === 1, 'Desktop project navigation unexpectedly wraps');
    for (const baselines of rows.values()) {
      check(Math.max(...baselines) - Math.min(...baselines) < 1, `Project navigation baselines differ at ${width}`);
    }
    const menu = page.locator('.community-menu');
    if (width === 390) {
      check(!await menu.evaluate(node => node.open), 'Mobile community menu should start closed');
      await menu.locator('summary').focus();
      await page.keyboard.press('Enter');
      check(await menu.evaluate(node => node.open), 'Keyboard must open the community menu');
      await page.keyboard.press('Escape');
      check(!await menu.evaluate(node => node.open), 'Escape must close the community menu');
    }
    await page.locator('.project-navigation').getByRole('link', { name: 'Get Started', exact: true }).click();
    check(new URL(page.url()).pathname.endsWith('/adopt/'), 'Relative project navigation failed');
    check(await page.locator('code.language-json span').count() > 0, 'JSON example lost highlighting');
    await page.evaluate(() => document.fonts.ready);
    check(!await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), `Guide overflows at ${width}`);
  }
  check(errors.length === 0, `Uncaught browser errors: ${errors.join('; ')}`);
  console.log('Project layout, nested links, JSON highlighting, responsive menu and overflow checks passed.');
}
