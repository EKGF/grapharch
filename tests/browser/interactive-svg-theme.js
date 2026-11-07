async (page) => {
  const diagram = page.locator('object.interactive-svg').first();
  await diagram.waitFor();
  await page.waitForFunction(() => document.querySelector('object.interactive-svg')?.contentDocument?.querySelector('.level-row'));
  const fill = () => diagram.evaluate(object => object.contentWindow.getComputedStyle(object.contentDocument.querySelector('.level-row')).fill);
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'light'; });
  await page.waitForTimeout(100);
  const light = await fill();
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'dark'; });
  await page.waitForTimeout(100);
  const dark = await fill();
  if (light === dark) throw new Error(`Interactive SVG ignores explicit publication theme: ${light}`);
  await page.evaluate(() => { document.documentElement.dataset.colorScheme = 'light'; });
  await page.waitForTimeout(100);
  if (await fill() !== light) throw new Error('Interactive SVG theme does not round-trip');
  return { light, dark };
}
