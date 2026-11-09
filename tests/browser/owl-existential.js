async (page) => {
  const figure = page.locator('figure.owl-class-diagram').filter({has: page.locator('figcaption', {hasText: /document\s*part/i})}).first();
  if (await figure.count() !== 1) throw new Error('Actual Commons DocumentPart diagram missing');
  const svg = figure.locator('svg');
  const text = (await svg.locator('text').allTextContents()).join(' ').replace(/\s+/g, ' ');
  if (!text.includes('1..*') || !text.includes('{qualified}') || !text.toLowerCase().includes('document') || !text.replace(/\s+/g,'').toLowerCase().includes('ismemberof')) throw new Error('Qualified existential association missing');
  for (const machinery of ['Anonymous', 'onProperty', 'someValuesFrom']) {
    if (text.includes(machinery)) throw new Error(`Unsupported visible existential notation: ${machinery}`);
  }
  const description = await svg.locator('desc').textContent();
  if (!description.includes('someValuesFrom') || !description.includes('other values are not excluded')) throw new Error('Accessible description lost original assertion or qualified meaning');
  const bounds = await svg.evaluate(node => {
    const result = {};
    for (const group of node.querySelectorAll('g.entity')) {
      const label = [...group.querySelectorAll('text')].map(n=>n.textContent).join('').replace(/\s+/g,'').toLowerCase();
      if (label.endsWith('documentpart')) result.part = group.getBBox();
      else if (label.endsWith('document')) result.document = group.getBBox();
    }
    return Object.fromEntries(Object.entries(result).map(([name,b])=>[name,{y:b.y,height:b.height}]));
  });
  if (!bounds.part || !bounds.document || bounds.document.y + bounds.document.height >= bounds.part.y) throw new Error('Superclass must stay above DocumentPart despite shared association target');
  for (const width of [1280, 390]) {
    await page.setViewportSize({width,height:900});
    await figure.scrollIntoViewIfNeeded();
    if (await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth)) throw new Error('Document overflows');
    await figure.locator('[tabindex="0"]').focus();
    await page.keyboard.press('ArrowRight');
  }
  return {qualifiedExistential:true, originalAssertionsAccessible:true, superclassAbove:true, viewports:[1280,390]};
}
