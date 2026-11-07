async (page) => {
  await page.waitForFunction(() => document.querySelector('object.interactive-svg')?.contentDocument?.querySelector('.neighbor-label'));
  const object = page.locator('object.interactive-svg').first();
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'light'; });
  await page.waitForFunction(() => {
    const object = document.querySelector('object.interactive-svg');
    return object.contentWindow.getComputedStyle(object.contentDocument.querySelector('.neighbor-label')).fill === 'rgb(39, 59, 92)';
  });
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'dark'; });
  await page.waitForFunction(() => {
    const object = document.querySelector('object.interactive-svg');
    return object.contentWindow.getComputedStyle(object.contentDocument.querySelector('.neighbor-label')).fill === 'rgb(219, 234, 254)';
  });
  const expected = await object.evaluate(object => {
    const anchor = object.contentDocument.querySelector('.neighbor-label').parentElement;
    if (anchor.localName !== 'a' || anchor.getAttribute('target') !== '_top') throw new Error('Native node must navigate its owning publication');
    return new URL(anchor.getAttribute('href'), object.contentDocument.baseURI).href;
  });
  const frame = page.frames().find(frame => frame.url().includes('/assets/interactive-'));
  if (!frame) throw new Error('Retained interactive diagram frame is missing');
  await frame.locator('.neighbor-label').first().click();
  await page.waitForURL(expected);
  if (!await page.locator('article').count()) throw new Error('Native node link did not open its authored page');
  return { destination: page.url(), theme: 'light and dark verified' };
}
