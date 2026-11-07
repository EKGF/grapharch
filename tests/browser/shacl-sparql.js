async page => {
  const codes = page.locator('pre > code.language-sparql');
  if (await codes.count() !== 5) throw new Error('Expected four actual ScheduleShape SELECTs and the ASK regression');
  const source = await codes.allTextContents();
  if (!source.some(code => code.includes('\n      WHERE {\n        $this'))) throw new Error('SHACL query indentation was collapsed');
  if (!source.some(code => code.includes('\n  ASK {\n    $this'))) throw new Error('ASK source lines were lost');
  if (!source.some(code => code.split('\n').some(line => line.length > 300))) throw new Error('The original long regex line is missing');
  if (await page.locator('td pre').count()) throw new Error('Code must not be squeezed into a predicate table cell');
  const link = page.locator('a[href*="sparql-query-"]').first();
  const target = await link.getAttribute('href');
  await link.focus();
  await page.keyboard.press('Enter');
  if (!await page.evaluate(target => document.getElementById(target.slice(1)) !== null, target)) throw new Error('Query link target missing');
  for (const width of [1280, 390]) {
    await page.setViewportSize({width, height: 900});
    const metrics = await page.evaluate(() => ({overflow: document.documentElement.scrollWidth > innerWidth, scrollableCode: [...document.querySelectorAll('pre')].some(pre => pre.scrollWidth > pre.clientWidth && getComputedStyle(pre).overflowX === 'auto')}));
    if (metrics.overflow || !metrics.scrollableCode) throw new Error(`Long code layout failed at ${width}px: ${JSON.stringify(metrics)}`);
  }
  return {codeBlocks: source.length, preservedIndentation: true, keyboardLink: target, desktopAndMobile: true};
}
