async (page) => {
  const base = new URL('./', page.url()).href;
  await page.setViewportSize({width: 1280, height: 900});
  await page.goto(base);
  const title = await page.locator('article h1').first().textContent();
  if (title !== 'MkDocs Bootstrap Theme') throw new Error('Original upstream heading was lost');
  const first = await page.locator('.primary-navigation a').first().textContent();
  if (first !== 'Home') throw new Error('The discovered homepage must lead default navigation');
  const images = await page.locator('article img').evaluateAll(nodes => nodes.map(node => node.src));
  if (images.length !== 20) throw new Error('Expected all twenty original theme screenshots');
  for (const source of images) {
    const response = await page.request.get(source);
    if (!response.ok() || !response.headers()['content-type'].startsWith('image/')) throw new Error(`Missing retained screenshot: ${source}`);
  }
  await page.getByRole('button', {name: 'Search documentation'}).click();
  await page.getByRole('searchbox').fill('Theme Examples');
  const result = page.locator('.site-search-results a').first();
  await result.waitFor();
  await result.click();
  await page.waitForURL('**/examples/');
  if (!(await page.locator('article').textContent()).includes('def fib():')) throw new Error('Original Python example was lost');
  await page.goto(new URL('sub-dir/sub-page/', base).href);
  if (!(await page.locator('article h1').textContent()).includes('Sub Page 1')) throw new Error('Nested documentation route is missing');
  await page.setViewportSize({width: 390, height: 844});
  await page.goto(base);
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error('Imported site overflows narrow screen');
  return {sourcePages: 4, retainedScreenshots: images.length, nativeMetadata: true, localSearch: true, nestedRoute: true, mobileOverflow: false};
}
