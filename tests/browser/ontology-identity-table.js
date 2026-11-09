async (page) => {
  const url=page.url().split('#')[0], results=[];
  for(const width of [1280,390]) {
    await page.setViewportSize({width,height:900});
    await page.goto(url);
    const table=page.locator('table.rdf-properties').filter({has:page.locator('thead tr:nth-child(2)',{hasText:/\/AuthorizedParty$/})});
    if(await table.count()!==1) throw Error('AuthorizedParty identity table missing');
    const rows=table.locator('thead > tr');
    if(await rows.count()!==3) throw Error('Two identity rows must precede the column header');
    for(let i=0;i<2;i++) if(await rows.nth(i).locator('td[colspan="2"]').count()!==1) throw Error('Identity row must span both columns');
    if(!(await rows.nth(2).innerText()).includes('RDF predicate')) throw Error('Predicate header displaced');
    if(!await table.locator('tbody tr').filter({hasText:'rdfs:label'}).count()) throw Error('Original label predicate missing');
    const redundant=await page.locator('main p').evaluateAll(nodes=>nodes.filter(n=>/^(Label|Identifier|IRI):/.test(n.textContent.trim())).map(n=>n.textContent));
    if(redundant.length) throw Error('Standalone identity or label paragraphs remain: '+redundant);
    await table.scrollIntoViewIfNeeded();
    const geometry=await table.evaluate(table=>({width:table.getBoundingClientRect().width,identityWidth:table.querySelector('thead td').getBoundingClientRect().width,pageWidth:document.documentElement.scrollWidth,viewport:innerWidth}));
    if(Math.abs(geometry.width-geometry.identityWidth)>2||geometry.pageWidth>width+1) throw Error(JSON.stringify(geometry));
    results.push({width,...geometry});
  }
  return results;
}
