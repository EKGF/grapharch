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
    const link=family.locator('a').filter({hasText:/Parties Ontology/}).first();
    const href=await link.getAttribute('href');
    await link.focus();await page.keyboard.press('Enter');await page.waitForLoadState('domcontentloaded');
    if(!page.url().endsWith(href))throw new Error('Keyboard library navigation failed');
    await page.locator('.family-navigation summary').click();
    await page.waitForFunction(()=>document.querySelector('.family-navigation a[aria-current=page]'));
    const local=page.locator('.page-outline-navigation a[href^="#"]').first();
    await local.click();
    if(await page.locator('.family-navigation a[aria-current=page]').count()!==1)throw new Error('Section navigation erased current library page');
    if(await page.locator('.page-outline-navigation a[aria-current=location]').count()!==1)throw new Error('Current section not marked');
    await page.locator('.family-navigation summary').scrollIntoViewIfNeeded();
    await page.screenshot({path:`output/playwright/library-navigation-${width}.png`});
    results.push({width,...layout});
  }
  return results;
}
