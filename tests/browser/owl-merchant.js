async (page) => {
  const url=page.url().split('#')[0], results=[];
  for(const width of [1280,390]) {
    await page.setViewportSize({width,height:900});await page.goto(url);
    const figure=page.locator('figure.owl-class-diagram').filter({has:page.locator('figcaption',{hasText:/^Class neighborhood: merchant \[/i})});
    if(await figure.count()!==1)throw Error('Merchant diagram missing');
    const svg=figure.locator('svg'), labels=await svg.locator('text').allTextContents();
    if(labels.filter(t=>t==='1..*').length!==2)throw Error('Two qualified target minima required');
    if(labels.some(t=>/OWL expression|onProperty|someValuesFrom|Literal value/.test(t)))throw Error('Raw restriction graph remains visible');
    for(const property of ['isClassifiedBy','isIdentifiedBy'])if(await svg.locator(`a[title$="/${property}"]`).count()!==1)throw Error('Property source link missing: '+property);
    const geometry=await svg.evaluate(node=>{
      const entities=[...node.querySelectorAll('g.entity')];
      const box=name=>entities.find(e=>e.querySelector('a')?.getAttribute('title')?.endsWith('/'+name)).getBoundingClientRect();
      const focus=box('Merchant'),parent=box('FunctionalBusinessEntity');
      const targets=['MerchantCategoryCode','MerchantIdentifier'].map(box);
      const counts=[...node.querySelectorAll('text')].filter(t=>t.textContent==='1..*').map(t=>t.getBoundingClientRect());
      const distance=(a,b)=>Math.hypot(a.x+a.width/2-b.x-b.width/2,a.y+a.height/2-b.y-b.height/2);
      return {parentAbove:parent.bottom<focus.top,countsAtTargets:targets.every(t=>counts.some(c=>distance(c,t)<distance(c,focus)))};
    });
    if(!geometry.parentAbove||!geometry.countsAtTargets)throw Error(JSON.stringify(geometry));
    if(!(await svg.locator('desc').textContent()).includes('someValuesFrom'))throw Error('Original facts missing');
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1))throw Error('Page overflow');
    results.push({width,...geometry});
  }
  return results;
}
