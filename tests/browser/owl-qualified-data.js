async (page) => {
  const url = page.url().split('#')[0], results = [];
  for (const width of [1280, 390]) {
    await page.setViewportSize({width, height:900});
    await page.goto(url);
    const figure = page.locator('figure.owl-class-diagram').filter({has:page.locator('figcaption', {hasText:/Class neighborhood: time of day /i})});
    if (await figure.count() !== 1) throw new Error('TimeOfDay diagram missing');
    const svg = figure.locator('svg');
    const text = (await svg.locator('text').allTextContents()).join(' ');
    if (!text.includes('has time value : xsd:string [1]')) throw new Error('Declared datatype/count member missing: '+text);
    if (/OWL expression|Literal value|onDataRange|qualifiedCardinality/.test(text)) throw new Error('Raw datatype restriction remains visible');
    const desc = await svg.locator('desc').textContent();
    for (const fact of ['onDataRange', 'qualifiedCardinality', 'xsd:string']) if (!desc.includes(fact)) throw new Error('Retained assertion missing: '+fact);
    if (await svg.locator('a[title$="/hasTimeValue"]').count() !== 1) throw new Error('Datatype property source link missing');
    const hierarchy = await svg.evaluate(node => {
      const entities = [...node.querySelectorAll('g.entity')];
      const box = name => entities.find(e => e.querySelector('a')?.getAttribute('title')?.endsWith('/'+name)).getBoundingClientRect();
      const parent = box('TimeInstant'), focus = box('TimeOfDay');
      return parent.bottom < focus.top;
    });
    if (!hierarchy) throw new Error('TimeInstant must remain above TimeOfDay');
    if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth+1)) throw new Error('Page overflow');
    results.push({width, hierarchy, datatypeMember:true});
  }
  return results;
}
