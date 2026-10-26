async (page) => {
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  await page.setViewportSize({width:1440,height:900});
  await page.getByRole('button',{name:'Collapse all',exact:true}).click();
  assert(await page.locator('.toc-toggle[aria-expanded=false]').count()>0,'no collapsed branches');
  await page.getByRole('button',{name:'Expand all',exact:true}).click();
  assert(await page.locator('.toc-toggle[aria-expanded=false]').count()===0,'expand failed');
  await page.keyboard.press('Control+k');
  await page.locator('#spec-search-query').fill('model');
  await page.locator('#spec-search-kind').selectOption('section');
  assert((await page.locator('#spec-search-results').textContent()).toLowerCase().includes('model'),'single-page heading search failed');
  await page.keyboard.press('Escape');
  await page.waitForFunction(()=>!document.getElementById('spec-search').open);
  assert(await page.locator('[data-search-open]').evaluate(e=>e===document.activeElement),'close did not restore focus');
  await page.screenshot({path:'output/playwright/specification-sidebar-desktop.png'});
  await page.setViewportSize({width:390,height:844});
  await page.getByRole('button',{name:/Search & filter/}).click();
  assert(await page.locator('#spec-search').isVisible(),'mobile dialog missing');
  assert(await page.locator('#spec-search').evaluate(e=>e.getBoundingClientRect().right<=innerWidth),'dialog overflows mobile screen');
  await page.keyboard.press('Escape');
  await page.waitForFunction(()=>!document.getElementById('spec-search').open);
  return {singlePageSearch:true,collapse:true,focus:true,mobile:true};
}
