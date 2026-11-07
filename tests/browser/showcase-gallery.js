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
  const outputs = await root.locator('a[data-artifact]').count();
  if (outputs < count * 3) throw new Error('Publication downloads missing');
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
