async (page) => {
  const url = page.url();
  const context = page.context();
  const results = [];
  for (const width of [1440, 390]) {
    const page = await context.newPage();
    await page.setViewportSize({ width, height: 844 });
    await page.goto(url);
    const top = await page.locator('.showcase-card').first().evaluate(node => node.getBoundingClientRect().top);
    if (top >= 844) throw new Error(`Cards hidden below initial ${width}px viewport: ${top}`);
    const browse = page.locator('.site-navigation > details');
    if (width === 390) {
      if (await browse.getAttribute('open') !== null) throw new Error('Mobile gallery rail starts expanded');
      await browse.locator(':scope > summary').click();
      await page.reload();
      if (await browse.getAttribute('open') === null) throw new Error('Explicit mobile navigation preference lost');
    }
    results.push({ width, top });
    await page.close();
  }
  return results;
}
