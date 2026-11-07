async (page) => {
  const button = page.getByRole('button', {name: 'Search documentation', exact: true});
  await button.click();
  const query = await page.evaluate(async () => {
    const dialog = document.querySelector('.site-search');
    const response = await fetch(new URL(dialog.dataset.searchIndex, location.href));
    const entries = await response.json();
    return entries[0].title.split(/\s+/)[0];
  });
  await page.getByLabel('Search all pages', {exact: true}).fill(query);
  await page.locator('.site-search-results a').first().waitFor();
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => !document.querySelector('.site-search').open, null, {timeout: 2000});
  if (!await button.evaluate(element => element === document.activeElement)) throw new Error('Search did not restore trigger focus');
  await page.keyboard.press('Control+k');
  await page.getByLabel('Search all pages', {exact: true}).fill(query);
  const link = page.locator('.site-search-results a').first();
  const target = await link.getAttribute('href');
  await link.click();
  await page.waitForURL(target);
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error('Documentation search causes horizontal overflow');
  return {query, target};
}
