async (page) => {
  const base=page.url().split('#')[0];
  const results=[];
  for(const width of [2048,1280,390]){
    await page.setViewportSize({width,height:900});
    await page.goto(base);
    const family=page.locator('.family-navigation');
    const summary=family.locator('summary');
    await summary.focus(); await page.keyboard.press('Enter');
    await page.waitForFunction(()=>document.querySelector('[data-family-tree] a'));
    const layout=await family.evaluate(n=>{
      const link=[...n.querySelectorAll('[data-family-tree] a')].find(a=>a.textContent.length>25);
      const outline=document.querySelector('.page-outline-navigation');
      return {display:getComputedStyle(link).display,width:link.getBoundingClientRect().width,sidebar:n.getBoundingClientRect().width,beforeOutline:!!outline && !!(n.compareDocumentPosition(outline)&Node.DOCUMENT_POSITION_FOLLOWING),overflow:document.documentElement.scrollWidth>innerWidth+1};
    });
    if(layout.display!=='block'||layout.width<layout.sidebar*.65)throw new Error(`Library links squeezed at ${width}: ${JSON.stringify(layout)}`);
    if(!layout.beforeOutline)throw new Error('Library switcher follows page outline');
    if(layout.overflow)throw new Error('Library navigation causes page overflow');
    const link=family.locator('a').filter({hasText:/Parties and Situations/}).first();
    const href=await link.getAttribute('href');
    await link.focus();await page.keyboard.press('Enter');await page.waitForURL(url=>url.pathname.endsWith(href));
    if(!page.url().endsWith(href))throw new Error('Keyboard library navigation failed');
    await page.locator('.family-navigation summary').click();
    await page.waitForFunction(()=>document.querySelector('.family-navigation a[aria-current=page]'));
    if(!(await page.locator('.family-navigation a[aria-current=page]').evaluate(a=>{const box=a.getBoundingClientRect(), viewport=a.closest('[data-family-tree]').getBoundingClientRect();return box.top>=viewport.top && box.bottom<=viewport.bottom;})))throw new Error('Selected ontology is hidden outside the library scroll viewport');
    const local=page.locator('.page-outline-navigation a[href^="#"]').first();
    await local.click();
    await page.waitForFunction(()=>document.querySelector('.page-outline-navigation a[aria-current=location]'));
    if(await page.locator('.family-navigation a[aria-current=page]').count()!==1)throw new Error('Section navigation erased current library page');
    if(await page.locator('.page-outline-navigation a[aria-current=location]').count()!==1)throw new Error('Current section not marked');
    await page.keyboard.press('Control+k');
    await page.locator('#spec-search-kind').selectOption('ontology');
    await page.locator('#spec-search-query').fill('Parties and Situations');
    await page.waitForFunction(()=>document.querySelector('#spec-search-results a'));
    if(!(await page.locator('#spec-search-results').textContent()).includes('Parties and Situations'))throw new Error('Library ontology search failed');
    await page.keyboard.press('Escape');
    await page.locator('.family-navigation summary').scrollIntoViewIfNeeded();
    await page.screenshot({path:`output/playwright/library-navigation-${width}.png`});
    results.push({viewport:width,...layout});
  }
  return results;
}
