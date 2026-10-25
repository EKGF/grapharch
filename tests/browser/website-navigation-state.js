async (page) => {
  const check = (value, message) => { if (!value) throw new Error(message); };
  const origin = new URL(page.url()).origin;
  // A new tab has fresh session storage; clearing a live page would be
  // undone by its pagehide snapshot during reload.
  page = await page.context().newPage();
  await page.addInitScript(() => {
    document.addEventListener('DOMContentLoaded', () => {
      const style = document.createElement('style');
      style.textContent = '::-webkit-scrollbar{width:16px}';
      document.head.append(style);
    });
  });
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.goto(`${origin}/usage/ontology-families/`);
  const navigation = () => page.locator('.site-navigation nav');
  const branch = name => navigation().locator('details').filter({ has: page.locator(':scope > summary > a').filter({ hasText: new RegExp(`^${name}$`) }) }).first();
  const toggle = async name => {
    const before = page.url();
    await branch(name).locator(':scope > summary').click({ position: { x: 8, y: 12 } });
    check(page.url() === before, 'A disclosure arrow must not navigate');
  };
  const otherPosition = () => navigation().getByRole('link', { name: 'Other', exact: true }).boundingBox();
  check(await branch('Usage').evaluate(node => node.open), 'Fresh deep links must reveal current ancestors');
  const initialUrl = page.url();
  await branch('Usage').locator(':scope > summary').focus();
  await page.keyboard.press('Space');
  check(!await branch('Usage').evaluate(node => node.open) && page.url() === initialUrl, 'Keyboard disclosure must toggle without navigation');
  await page.keyboard.press('Space');
  const before = await otherPosition();
  await navigation().getByRole('link', { name: 'Showcase gallery', exact: true }).click();
  await page.waitForURL('**/showcases/');
  const gallery = await otherPosition();
  check(Math.abs(before.x - gallery.x) < 1, `Sidebar moved horizontally: ${before.x} to ${gallery.x}`);
  check(Math.abs(before.y - gallery.y) < 1, `Other jumped after gallery navigation: ${before.y} to ${gallery.y}`);
  check(await branch('Usage').evaluate(node => node.open), 'Open branch state was lost across pages');
  await toggle('Usage');
  await toggle('Other');
  const closed = await otherPosition();
  await branch('Usage').locator(':scope > summary > a').click();
  await page.waitForURL('**/usage/');
  check(!await branch('Usage').evaluate(node => node.open), 'Saved closed branch must override current-page expansion');
  check(await branch('Other').evaluate(node => node.open), 'Other branch state was lost');
  const after = await otherPosition();
  check(Math.abs(closed.y - after.y) < 1, 'Previously closed branches changed geometry on navigation');
  await page.reload();
  check(!await branch('Usage').evaluate(node => node.open), 'Closed state was lost on reload');
  await navigation().getByRole('link', { name: 'Other', exact: true }).click();
  await page.waitForURL('**/other/');
  const other = await otherPosition();
  check(Math.abs(after.x - other.x) < 1 && Math.abs(after.y - other.y) < 1, 'Short page changed sidebar geometry');
  await toggle('Other');
  await page.goBack();
  check(!await branch('Other').evaluate(node => node.open), 'Back navigation restored stale expanded state');
  // Exercise both desktop sidebar scrolling and the independently scrolling mobile nav.
  for (const width of [1600, 390]) {
    await page.setViewportSize({ width, height: 600 });
    await page.locator('.site-navigation > details').evaluate(node => { node.open = true; });
    await navigation().locator('details').evaluateAll(nodes => nodes.forEach(node => { node.open = true; }));
    const scroller = width > 760 ? page.locator('.site-navigation') : navigation();
    await scroller.evaluate(node => { node.scrollTop = 125; });
    const scroll = await scroller.evaluate(node => node.scrollTop);
    check(scroll > 0, 'Fixture must overflow its sidebar scroll container');
    await page.reload();
    check(Math.abs(await scroller.evaluate(node => node.scrollTop) - scroll) < 1, `Sidebar scroll lost on reload at ${width}px`);
  }
  await page.locator('.site-navigation > details > summary').click();
  check(!await page.locator('.site-navigation > details').evaluate(node => node.open), 'Mobile browse control must close');
  await page.setViewportSize({ width: 1600, height: 900 });
  await navigation().waitFor({ state: 'visible' });
  check(await navigation().isVisible(), 'Resizing from closed mobile navigation must reveal desktop navigation');
  await page.close();
  return { keyboard: true, responsive: true, freshDeepLink: true, branchPersistence: true, closedState: true, geometry: true, arrow: true, desktopScroll: true, mobileScroll: true };
}
