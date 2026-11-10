async (page) => {
  const root = page.locator('[data-showcase-gallery]');
  const cards = root.locator('.showcase-card');
  const count = await cards.count();
  if (!count) throw new Error('Gallery is empty');
  const input = root.locator('#showcase-search');
  const format = root.locator('[data-gallery-filter="input"]');
  await input.focus();
  await page.keyboard.press('Tab');
  if (!(await format.evaluate(node => node === document.activeElement))) throw new Error('Keyboard order skips source-format filter');
  await format.selectOption('OWL');
  const filtered = root.locator('.showcase-card:visible');
  const visible = await filtered.count();
  if (!visible || visible >= count) throw new Error('OWL filter does not narrow the gallery');
  for (const item of await filtered.all()) {
    if (!(await item.getAttribute('data-input')).split('|').includes('OWL')) throw new Error('Unrelated card survives OWL filter');
  }
  if (!(new URL(page.url())).searchParams.get('showcase-input')) throw new Error('Filter state missing from URL');
  await page.reload();
  if (await format.inputValue() !== 'OWL') throw new Error('Reload lost filter state');
  await input.fill('no-matching-publication-98471');
  if (await root.locator('.showcase-card:visible').count() !== 0) throw new Error('Empty result search failed');
  if (!(await root.locator('[data-gallery-empty]').isVisible())) throw new Error('No-results explanation missing');
  await root.getByRole('button', { name: 'Clear filters' }).click();
  if (await root.locator('.showcase-card:visible').count() !== count) throw new Error('Reset did not restore all showcases');
  if (!(await input.evaluate(node => node === document.activeElement))) throw new Error('Reset should focus search');
  const mkdocs = root.locator('.showcase-card[data-showcase="urn:grapharch:showcase:mkdocs-bootswatch"]');
  if (await mkdocs.count()) {
    await format.selectOption('MkDocs');
    if (await root.locator('.showcase-card:visible').count() !== 1) throw new Error('MkDocs source filter is not specific');
    const output = root.locator('[data-gallery-filter="output"]');
    await output.selectOption('PDF');
    if (await mkdocs.isVisible()) throw new Error('HTML-only MkDocs card falsely advertises PDF');
    await output.selectOption('HTML');
    if (!(await mkdocs.isVisible())) throw new Error('MkDocs HTML publication is hidden');
    await root.getByRole('button', { name: 'Clear filters' }).click();
  }
  const fibo = root.locator('.showcase-card[data-showcase="urn:grapharch:showcase:fibo-be"]');
  if (await fibo.count()) {
    await input.fill('FIBO Business Entities');
    if (await root.locator('.showcase-card:visible').count() !== 1 || !(await fibo.isVisible())) throw new Error('FIBO search does not find the complete BE example');
    const outputs = (await fibo.getAttribute('data-output')).split('|');
    for (const output of ['HTML', 'PDF', 'KG']) if (!outputs.includes(output)) throw new Error(`FIBO ${output} output missing`);
    if (!(await fibo.textContent()).includes('48')) throw new Error('FIBO source scope missing');
    await root.getByRole('button', { name: 'Clear filters' }).click();
  }
  if (await root.getByText('Open HTML', { exact: true }).count()) throw new Error('Redundant Open HTML button remains');
  const galleryUrl = page.url();
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    const card = cards.first();
    const primary = card.locator('a.showcase-primary');
    if (await primary.count() !== 1 || await card.locator('a a').count()) throw new Error('Card link structure is invalid');
    const target = await primary.evaluate(node => node.href);
    await card.scrollIntoViewIfNeeded();
    const box = await card.boundingBox();
    await page.mouse.click(box.x + 8, box.y + 8);
    await page.waitForURL(target);
    await page.goto(galleryUrl);
    await primary.focus();
    const outline = await primary.evaluate(node => getComputedStyle(node).outlineStyle);
    if (outline === 'none') throw new Error('Primary card focus is invisible');
    await page.keyboard.press('Enter');
    await page.waitForURL(target);
    const detail = page.locator('.showcase-detail');
    if (!(await detail.locator('h1').textContent())) throw new Error('Missing introduction title');
    const publication = detail.locator('.showcase-outputs a[href$=".html"]').first();
    if (!(await publication.isVisible())) throw new Error('Missing primary HTML output');
    if (await page.locator('.site-navigation').count()) throw new Error('Detail shows documentation navigation');
    if (!(await page.locator('.breadcrumbs').innerText()).startsWith('Showcase')) throw new Error('Wrong detail breadcrumb');
    const hrefs = await detail.locator('a[data-artifact]').evaluateAll(nodes => nodes.map(n => n.href));
    if (new Set(hrefs).size !== hrefs.length) throw new Error('Duplicate publication outputs');
    const margins = await detail.locator('.showcase-tags li').evaluateAll(nodes => nodes.map(n => getComputedStyle(n).marginTop));
    if (margins.some(margin => margin !== '0px')) throw new Error('Inconsistent tag spacing');
    for (const link of await detail.locator('.showcase-outputs a').all()) {
      await link.scrollIntoViewIfNeeded();
      await link.click({ trial: true });
    }
    await page.locator('.primary-navigation').getByRole('link', { name: 'Gallery', exact: true }).click();
    await page.waitForURL(url => url.pathname.replace(/index\.html$/, '') === new URL(galleryUrl).pathname.replace(/index\.html$/, ''));
    await page.goto(galleryUrl);
  }
  const outputs = await root.locator('a[data-artifact]').count();
  if (outputs !== 0) throw new Error('Gallery still contains output controls');
  await page.setViewportSize({ width: 390, height: 844 });
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error('Mobile gallery overflows');
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'dark'; });
  const dark = await cards.first().evaluate(node => getComputedStyle(node).backgroundColor);
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'light'; });
  const light = await cards.first().evaluate(node => getComputedStyle(node).backgroundColor);
  if (dark === light) throw new Error('Gallery ignores theme selection');
  await page.setViewportSize({ width: 1440, height: 1000 });
  return { showcases: count, owlMatches: visible, outputs, light, dark };
}
