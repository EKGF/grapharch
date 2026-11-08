async (page) => {
  const base = page.url().split('#')[0];
  const results = [];
  for (const width of [1280, 390]) {
    await page.setViewportSize({width, height:900});
    await page.goto(base);
    const figures = page.locator('figure.owl-class-diagram');
    const actual = figures.filter({has:page.locator('figcaption', {hasText:/government department/i})});
    if (await actual.count()) {
      const svg = actual.first().locator('svg');
      const visible = (await svg.locator('text').allTextContents()).join(' ');
      if (/expression|minQualifiedCardinality|onClass|onProperty|government minister/i.test(visible)) throw new Error('Tautological zero restriction still visible');
      const description = await svg.locator('desc').textContent();
      if (!description.includes('minQualifiedCardinality') || !description.includes('zero qualified minimum')) throw new Error('Original zero restriction or accessible qualification lost');
    } else {
      const positive = figures.filter({has:page.locator('figcaption', {hasText:/^Class neighborhood: Department\s/})});
      if (await positive.count() !== 1) throw new Error('Positive Department fixture missing');
      const svg = positive.locator('svg');
      const labels = await svg.locator('text').allTextContents();
      if (!labels.includes('1..*') || !labels.includes('{qualified}')) throw new Error('Qualified target multiplicity missing');
      const geometry = await svg.evaluate(node => {
        const label = [...node.querySelectorAll('text')].find(n=>n.textContent==='1..*').getBoundingClientRect();
        const entities = [...node.querySelectorAll('g.entity')];
        const target = entities.find(n=>n.textContent.includes('Minister')).getBoundingClientRect();
        const source = entities.find(n=>n.textContent.includes('Department')).getBoundingClientRect();
        const distance = box => Math.hypot(label.x+label.width/2-box.x-box.width/2,label.y+label.height/2-box.y-box.height/2);
        return {target:distance(target),source:distance(source)};
      });
      if (geometry.target >= geometry.source) throw new Error('Multiplicity is not at Minister endpoint');
    }
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1)) throw new Error('Page overflow');
    results.push({width,actualGovernmentDepartment:await actual.count()===1});
  }
  return results;
}
