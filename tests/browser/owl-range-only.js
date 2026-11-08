async (page) => {
  const figure = page.locator('figure.owl-class-diagram').filter({has:page.locator('figcaption',{hasText:/legally delegated authority/i})}).first();
  if (await figure.count() !== 1) throw new Error('Actual Commons legally delegated authority diagram missing');
  const svg = figure.locator('svg');
  const placeholder = svg.locator('g.entity[data-qualified-name="unknownDomain"]');
  if (await placeholder.count() !== 1 || !(await placeholder.locator('rect').evaluate(node => getComputedStyle(node).strokeDasharray !== 'none'))) throw new Error('Generic domain placeholder must have a dotted boundary');
  const texts = await svg.locator('text').allTextContents();
  if (!texts.includes('delegates control to')) throw new Error('Authored association label missing');
  if (texts.includes('ity') || texts.includes('legally delegated author')) throw new Error('An authored word was split in the class name');
  const property = svg.locator('a[title="https://www.omg.org/spec/Commons/BusinessAuthorizations/delegatesControlTo"]');
  if (await property.count() !== 1) throw new Error('Property identity tooltip missing');
  const href = await property.getAttribute('href');
  if (!href || !href.startsWith('#') || await page.locator(`[id="${href.slice(1)}"]`).count() !== 1) throw new Error('Property documentation link does not resolve');
  const description = await svg.locator('desc').textContent();
  if (!description.includes('presentation placeholder') || !description.includes('not an asserted domain')) throw new Error('Accessible qualification missing');
  for (const width of [1280,390]) {
    await page.setViewportSize({width,height:1000});
    await figure.scrollIntoViewIfNeeded();
    await figure.locator('[tabindex="0"]').focus();
    await page.keyboard.press('ArrowRight');
    if (await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)) throw new Error(`Page overflow at ${width}`);
  }
  return {authoredLabels:true,dottedPlaceholder:true,sourceIdentity:true,accessibleQualification:true,viewports:[1280,390]};
}
