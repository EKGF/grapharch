async (page) => {
  const failures = [];
  const check = (value, message) => { if (!value) failures.push(message); };
  await page.setViewportSize({ width: 1440, height: 1000 });
  const chapter = page.locator('main > section').filter({has: page.locator('h2', {hasText: 'Artifact Dependency Ontology'})});
  const overview = chapter.locator('h3').filter({hasText: 'Overview'});
  await overview.scrollIntoViewIfNeeded();
  const hierarchy = await chapter.locator('h3, h4').evaluateAll(nodes => nodes.map(node => ({tag:node.tagName, text:node.textContent})));
  check(hierarchy.some(h => h.tag === 'H3' && h.text.includes('Classes')), 'missing Classes section');
  check(hierarchy.some(h => h.tag === 'H3' && h.text.includes('Named Individuals')), 'missing Named Individuals section');
  check(hierarchy.some(h => h.tag === 'H4' && h.text.includes('Ambiguous')), 'individual must be nested');
  check(!hierarchy.some(h => h.text.includes('Named individual:')), 'repeated type prefix');
  const table = chapter.locator('table').first();
  check(await table.locator('tbody tr').count() === 2, 'duplicate ontology and namespace IRI');
  const ribbon = page.locator('.authority-banner');
  const style = await ribbon.evaluate(e => ({select:getComputedStyle(e).userSelect, cursor:getComputedStyle(e).cursor}));
  check(style.select === 'none' && style.cursor === 'default', 'selectable status ribbon');
  await page.screenshot({path:'output/reference-review/desktop.png'});
  const tocLink = page.locator('.toc a').filter({hasText:'Classes'}).first();
  check(await tocLink.count() === 1, 'missing section in contents');
  await tocLink.click();
  check(await page.locator(page.url().slice(page.url().indexOf('#'))).count() === 1, 'broken contents anchor');
  await page.setViewportSize({width:390,height:844});
  await overview.scrollIntoViewIfNeeded();
  check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), 'mobile horizontal overflow');
  await page.screenshot({path:'output/reference-review/mobile.png'});
  if (failures.length) throw new Error(failures.join('; '));
  return {passed:true, hierarchy:hierarchy.slice(0,8), ribbon:style};
}
