async (page) => {
  const base = page.url().split('#')[0], results=[];
  for (const width of [1280,390]) {
    await page.setViewportSize({width,height:900});
    await page.goto(base);
    const figure=page.locator('figure.owl-class-diagram').filter({has:page.locator('figcaption',{hasText:/Class neighborhood: (government official|Official) /})});
    if(await figure.count()!==1) throw new Error('Official class diagram missing');
    const svg=figure.locator('svg');
    const labels=await svg.locator('text').allTextContents();
    if(labels.some(t=>/OWL expression|Literal value|onProperty|qualifiedCardinality/.test(t)))throw new Error('Restriction machinery is still visible');
    if(!labels.includes('1')||!labels.includes('{qualified}'))throw new Error('Qualified exact cardinality missing');
    const description=await svg.locator('desc').textContent();
    for(const fact of ['someValuesFrom','minQualifiedCardinality','qualifiedCardinality'])if(!description.includes(fact))throw new Error('Retained original fact missing: '+fact);
    const geometry=await svg.evaluate(node=>{
      const entities=[...node.querySelectorAll('g.entity')].map(e=>({label:e.textContent.replace(/\s+/g,'').toLowerCase(),box:e.getBoundingClientRect()}));
      const focus=entities.find(e=>e.label.endsWith('governmentofficial')||e.label.endsWith('»official'));
      const parent=entities.find(e=>e.label.includes('responsibleparty'));
      const child=entities.find(e=>e.label.includes('appointee'));
      const target=entities.find(e=>e.label.includes('legallycompetentnaturalperson')||e.label.endsWith('»person'));
      const count=[...node.querySelectorAll('text')].find(e=>e.textContent==='1').getBoundingClientRect();
      const distance=box=>Math.hypot(count.x+count.width/2-box.x-box.width/2,count.y+count.height/2-box.y-box.height/2);
      return {parentAbove:parent.box.y+parent.box.height<focus.box.y,childBelow:child.box.y>focus.box.y+focus.box.height,targetCount:distance(target.box)<distance(focus.box)};
    });
    if(!geometry.parentAbove||!geometry.childBelow||!geometry.targetCount)throw new Error(JSON.stringify(geometry));
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1))throw new Error('Page overflow');
    results.push({width,...geometry});
  }
  return results;
}
