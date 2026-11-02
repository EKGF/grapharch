async (page) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  const home = page.url();
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(home);
    check(await page.locator('.project-hero').count() === 1, 'Project hero missing');
    check(await page.locator('.site-navigation').count() === 0, 'Documentation rail leaked into project layout');
    check(!await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), `Project overflows at ${width}`);
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
    check(!await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), `Guide overflows at ${width}`);
  }
  check(errors.length === 0, `Uncaught browser errors: ${errors.join('; ')}`);
  console.log('Project layout, nested links, JSON highlighting, responsive menu and overflow checks passed.');
}
