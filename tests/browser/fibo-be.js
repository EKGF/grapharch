async (page) => {
  const cards = page.locator('.ontology-card');
  if (await cards.count() !== 48) throw new Error('Complete 48-document BE inventory missing');
  const counts = await cards.evaluateAll(nodes => nodes.reduce((n,card) => ({classes:n.classes+Number(card.dataset.classCount), properties:n.properties+Number(card.dataset.propertyCount)}),{classes:0,properties:0}));
  if (counts.classes !== 199 || counts.properties !== 92) throw new Error(`BE-owned distinct resource counts disagree with pinned source inventory: ${JSON.stringify(counts)}`);
  const legal = cards.filter({hasText:'Legal Persons Ontology'}).getByRole('link');
  if (await legal.count() !== 1) throw new Error('Legal Persons ontology card missing');
  for (const width of [2048,1280,390]) {
    await page.setViewportSize({width,height:1000});
    if (await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth)) throw new Error(`BE homepage overflows at ${width}`);
  }
  await legal.focus();
  await page.keyboard.press('Enter');
  await page.waitForURL(/section-/);
  if (!(await page.title()).includes('Legal Persons')) throw new Error('Ontology card route incorrect');
  if (await page.locator('figure.owl-class-diagram').count() !== 12) throw new Error('Legal Persons class figures missing');
  const home = page.locator('.library-home-link a');
  if (await home.count() !== 1) throw new Error('Ontology page lacks domain home link');
  await home.click();
  await page.waitForURL(/index.html$/);
  return {ontologies:48, ...counts, ontologyNavigation:true, viewports:[2048,1280,390]};
}
