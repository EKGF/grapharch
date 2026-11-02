async (page) => {
  const check = (value, message) => { if (!value) throw new Error(message); };
  const results = [];
  for (const width of [1440, 1000, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.reload();
    const row = page.locator('.community-header-row');
    const menu = row.locator('.community-menu');
    const geometry = await row.evaluate(node => {
      const box = node.getBoundingClientRect();
      return { height: box.height, left: box.left, right: box.right, overflow: document.documentElement.scrollWidth > innerWidth };
    });
    check(geometry.height === (width > 1050 ? 80 : 72), `Incorrect community header height at ${width}: ${JSON.stringify(geometry)}`);
    check(!geometry.overflow, `Page overflow at ${width}`);
    check(await row.locator('.ekgf-identity').isVisible(), 'Missing EKGF identity');
    check(await page.locator('header .brand-version').isVisible(), 'Missing project version');
    if (width > 1050) {
      check(await menu.evaluate(node => node.open), 'Desktop navigation must be open');
      const logo = await row.locator('.ekgf-identity').boundingBox();
      const nav = await row.locator('nav').boundingBox();
      const tools = await row.locator('.site-links').boundingBox();
      check(logo.x + logo.width < nav.x && nav.x + nav.width <= tools.x, 'Header columns overlap');
    } else {
      check(!await menu.evaluate(node => node.open), 'Mobile menu should start closed');
      await menu.locator('summary').focus();
      await page.keyboard.press('Enter');
      check(await menu.locator('nav a').first().isVisible(), 'Keyboard must open navigation');
      await page.keyboard.press('Escape');
      check(!await menu.evaluate(node => node.open), 'Escape should close mobile menu');
      check(await menu.locator('summary').evaluate(node => node === document.activeElement), 'Escape should restore focus');
    }
    const brand = await page.locator('.project-identity-row .brand').boundingBox();
    check(brand.y >= geometry.height, 'Product identity must not interrupt community navigation');
    results.push({ width, ...geometry });
  }
  return results;
}
