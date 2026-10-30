async (page) => {
  const panel = page.locator('details.publication-preamble');
  if (await panel.count() !== 1) throw Error('missing legal disclosure');
  if (await panel.getAttribute('open') !== null) throw Error('legal disclosure starts expanded');
  await panel.locator('summary').focus();
  await page.keyboard.press('Enter');
  for (const selector of ['p', 'li']) {
    const style = await panel.locator(selector).first().evaluate(element => {
      const css = getComputedStyle(element);
      return { align: css.textAlign, last: css.textAlignLast };
    });
    if (style.align !== 'justify' || style.last !== 'left') {
      throw Error(`legal ${selector} must justify both edges and leave the last line left-aligned`);
    }
  }
  if (!(await panel.innerText()).includes('COMPLIANCE')) throw Error('incomplete legal text');
  await page.setViewportSize({width: 390, height: 844});
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw Error('mobile overflow');
  return {keyboard: true, justifiedParagraphs: true, justifiedClauses: true, mobile: true};
}
