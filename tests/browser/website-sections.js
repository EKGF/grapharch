async (page) => {
  const base = await page.locator(".brand").evaluate(node => node.href);
  const route = path => new URL(path.replace(/^\//, ""), base).href;
  const checks = [
    ['/', 'Overview'],
    ['/overview/', 'Overview'],
    ['/usage/', 'Documentation'],
    ['/features/owl-class-diagrams/', 'Documentation'],
    ['/design-goals/', 'Documentation'],
    ['/showcases/', 'Gallery'],
    ['/showcase-dprod.html', 'Gallery'],
  ];
  for (const [path, section] of checks) {
    await page.goto(route(path));
    const tabs = page.locator('.primary-navigation > ul > li > a');
    if ((await tabs.allTextContents()).join('|') !== 'Overview|Documentation|Gallery') throw new Error(`Wrong primary sections on ${path}`);
    if ((await page.locator('.primary-navigation .active-section').textContent()) !== section) throw new Error(`Incorrect active section on ${path}`);
    if (section === 'Gallery' && await page.locator('.site-navigation').count()) throw new Error('Gallery inherits documentation rail');
    if (section === 'Documentation') {
      const sidebar = await page.locator('.site-navigation').textContent();
      if (!sidebar.includes('Collectors') || sidebar.includes('Showcase gallery')) throw new Error('Documentation sidebar crosses sections');
    }
    if (!(await page.locator('.brand-version').isVisible())) throw new Error('Build identity disappeared');
  }
  await page.goto(route('/showcases/'));
  for (const width of [1440, 390]) {
    await page.setViewportSize({width,height:900});
    if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1)) throw new Error('Section navigator overflows');
    const overview = page.locator('.primary-navigation a').first();
    await overview.focus();
    if (await overview.evaluate(n => getComputedStyle(n).outlineStyle === 'none')) throw new Error('Section keyboard focus invisible');
    await page.keyboard.press('Enter');
    await page.waitForURL(route('/'));
    await page.goto(route('/showcases/'));
  }
  return {sections:3,deepLinks:checks.length,keyboard:true,responsive:true};
}
