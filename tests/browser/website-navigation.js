async (page) => {
  const check = (value, message) => { if (!value) throw new Error(message); };
  for (const width of [1440, 1000, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.reload();
    await page.locator('.site-navigation > details').evaluate(node => { node.open = true; });
    await page.locator('.site-navigation nav details').evaluateAll(nodes => nodes.forEach(node => { node.open = true; }));
    const groups = page.locator('.site-navigation nav ul');
    for (const group of await groups.all()) {
      const positions = await group.locator(':scope > li > a, :scope > li > details > summary > a').evaluateAll(nodes => nodes.map(node => {
        const range = document.createRange();
        range.selectNodeContents(node);
        return range.getBoundingClientRect().left;
      }));
      if (positions.length > 1) check(Math.max(...positions) - Math.min(...positions) < 1, `Sibling labels misaligned at ${width}px: ${positions}`);
    }
    const section = page.locator('.site-navigation nav > ul > li > details').first();
    await section.evaluate(node => { node.open = false; });
    const summary = section.locator(':scope > summary');
    const beforeToggle = page.url();
    await summary.click({ position: { x: 9, y: 12 } });
    check(page.url() === beforeToggle, 'Arrow navigated instead of toggling the section');
    check(await section.evaluate(node => node.open), 'Arrow did not expand section');
    const parent = await summary.locator('a').boundingBox();
    const child = await section.locator(':scope > ul a').first().boundingBox();
    check(child.x > parent.x, 'Children must be indented beneath their parent');
  }
  return { alignment: true, expansion: true, indentation: true, desktop: true, mobile: true };
}
