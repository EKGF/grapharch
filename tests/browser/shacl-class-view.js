async (page) => {
  await page.setViewportSize({width: 1280, height: 900});
  const figure = page.locator('.shacl-validation-diagram');
  if (await figure.count() !== 1) throw new Error('Expected a separate shape figure');
  const region = figure.getByRole('region');
  await region.focus();
  if (!await region.evaluate(node => node === document.activeElement)) throw new Error('Figure cannot receive keyboard focus');
  const link = page.getByRole('link', {name: 'Full shape definition', exact: true});
  const href = await link.getAttribute('href');
  if (!href.startsWith('#') || !await page.locator(href).count()) throw new Error('Shape definition link is not retained locally');
  await page.setViewportSize({width: 390, height: 844});
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error('Mobile document overflows');
  const minimumText = await figure.locator('svg text').evaluateAll(nodes => Math.min(...nodes.map(node => node.getBoundingClientRect().height)));
  if (minimumText < 9) throw new Error(`Shape text too small: ${minimumText}`);
  return {figures: 1, localShapeLink: href, mobileMinimumText: minimumText};
}
