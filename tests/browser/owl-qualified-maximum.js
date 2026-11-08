async (page) => {
  const base=page.url().split('#')[0], results=[];
  for(const width of [1280,390]) {
    await page.setViewportSize({width,height:900});await page.goto(base);
    const figure=page.locator('figure.owl-class-diagram').filter({has:page.locator('figcaption',{hasText:/explicit date period/i})});
    if(await figure.count()!==1)throw new Error('Explicit Date Period diagram missing');
    const svg=figure.locator('svg'), labels=await svg.locator('text').allTextContents();
    if(labels.filter(t=>t==='0..1').length!==3)throw new Error('Three qualified maxima must appear at association endpoints');
    if(labels.some(t=>/OWL expression|Literal value|onProperty|onClass|maxQualified/.test(t)))throw new Error('Restriction machinery remains visible');
    for(const name of ['hasStartDate','hasEndDate','hasDuration'])if(await svg.locator(`a[title$="/${name}"]`).count()!==1)throw new Error('Property identity link missing: '+name);
    const hierarchy=await svg.evaluate(node=>{
      const entities=[...node.querySelectorAll('g.entity')];
      const box=name=>entities.find(e=>e.querySelector('a')?.getAttribute('title')?.endsWith('/'+name)).getBoundingClientRect();
      const focus=box('ExplicitDatePeriod');
      return ['DatePeriod','ProperInterval'].every(name=>{const parent=box(name);return parent.y+parent.height<focus.y;});
    });
    if(!hierarchy)throw new Error('Both superclasses must remain above focal class');
    const desc=await svg.locator('desc').textContent();if(!desc.includes('maxQualifiedCardinality'))throw new Error('Original assertions missing');
    const region=figure.locator('[role="region"]');
    const dimensions=await region.evaluate(n=>({available:n.clientWidth,total:n.scrollWidth}));
    if(dimensions.total>dimensions.available+1){await region.focus();await page.keyboard.press('ArrowRight');await page.waitForFunction(()=>document.activeElement.scrollLeft>0);}
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1))throw new Error('Page overflow');
    results.push({width,...dimensions,hierarchy});
  }
  return results;
}
