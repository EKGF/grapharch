async (page) => {
  const root = new URL('./', page.url()).href;
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  const results = [];
  for (const width of [1440, 1000, 390]) {
    await page.setViewportSize({ width, height: 900 });
    for (const route of ['provenance.html', 'quality-issues.html']) {
      await page.goto(new URL(route, root).href);
      const geometry = await page.evaluate(() => {
        const main = document.querySelector('main#main-content').getBoundingClientRect();
        const title = document.querySelector('main h1');
        return {
          left: main.left, width: main.width,
          titleSize: parseFloat(getComputedStyle(title).fontSize),
          overflow: document.documentElement.scrollWidth > innerWidth,
        };
      });
      check(await page.locator('header.site-header .brand-version').count() === 1, `${route}: missing branded header`);
      check(await page.locator('.site-layout .site-navigation').count() === 1, `${route}: missing shared navigation`);
      check(await page.locator('main h1').count() === 1, `${route}: one page heading required`);
      check(geometry.left >= 16 && geometry.width <= 900, `${route}: unconstrained content at ${width}`);
      check(geometry.titleSize >= 28 && !geometry.overflow, `${route}: unreadable or overflowing at ${width}`);
      await page.getByRole('link', { name: 'Documentation home', exact: true }).click();
      check(page.url() === root, `${route}: footer home did not navigate to ${root}`);
      results.push({ route, viewport: width, ...geometry });
    }
  }
  return results;
}
