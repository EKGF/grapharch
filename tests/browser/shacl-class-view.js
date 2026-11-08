async (page) => {
  await page.setViewportSize({width: 1280, height: 900});
  if (await page.locator('.shacl-validation-diagram').count()) throw new Error('Separate shape diagram should not be generated');
  const summary = page.locator('table.shacl-properties');
  if (await summary.count() !== 1) throw new Error('Expected a compact shape summary');
  const firstColumn = await summary.locator('tbody tr td:first-child').evaluateAll(nodes => nodes.map(node => ({width:node.getBoundingClientRect().width,height:node.getBoundingClientRect().height})));
  if (firstColumn.some(cell => cell.width < 180)) throw new Error('Property identifiers are squeezed into narrow cells');
  const details = page.locator('details').filter({has:page.locator('summary', {hasText:'Constraint details'})});
  await details.locator('summary').focus();
  await page.keyboard.press('Enter');
  if (!await details.evaluate(node => node.open)) throw new Error('Constraint details are not keyboard accessible');
  const link = page.getByRole('link', {name: 'Full shape definition', exact: true});
  const href = await link.getAttribute('href');
  if (!href.startsWith('#') || !await page.locator(href).count()) throw new Error('Shape definition link is not retained locally');
  const relationships = page.locator('.owl-relationships');
  if (await relationships.count()) {
    const x = await relationships.first().evaluate(node => node.getBoundingClientRect().x);
    const contentX = await summary.evaluate(node => node.closest('.table-scroll').getBoundingClientRect().x);
    if (Math.abs(x - contentX) > 1) throw new Error('Relationships are not aligned with content');
  }
  await page.setViewportSize({width: 390, height: 844});
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1)) throw new Error('Mobile document overflows');
  const region = summary.locator('..');
  await region.focus();
  await page.keyboard.press('ArrowRight');
  await page.waitForFunction(() => document.activeElement.scrollLeft > 0);
  return {separateShapeFigures:0, summaryColumns:4, localShapeLink:href, keyboardDetails:true, keyboardTableScroll:true, firstColumn};
}
