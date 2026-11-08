async (page) => {
  const base=page.url().split('#')[0], results=[];
  for(const width of [2048,1280,390]) {
    await page.setViewportSize({width,height:900}); await page.goto(base);
    const cards=page.locator('.ontology-card');
    if(await cards.count()!==24)throw new Error('Expected 24 Commons ontology cards');
    const layout=await page.evaluate(()=>({width:document.querySelector('main').getBoundingClientRect().width,overflow:document.documentElement.scrollWidth>innerWidth+1,badge:!!document.querySelector('.authority-banner'),narrow:[...document.querySelectorAll('.ontology-card')].some(n=>n.getBoundingClientRect().width<250)}));
    if(layout.overflow||layout.width>1201||layout.narrow||!layout.badge)throw new Error(JSON.stringify(layout));
    if(await page.locator('.family-navigation').count())throw new Error('Competing family navigator remains');
    await page.screenshot({path:`output/playwright/library-cards-${width}.png`,fullPage:false});
    const link=cards.locator('h2 a').filter({hasText:/Parties and Situations/}).first(), href=await link.getAttribute('href');
    await link.focus();await page.keyboard.press('Enter');await page.waitForURL(url=>url.pathname.endsWith(href));
    if(await page.locator('.ontology-card').count())throw new Error('Ontology is not an ordinary specification');
    const back=page.locator('.library-home-link a');
    if(await back.count()!==1)throw new Error('Missing library backlink');
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1))throw new Error('Ontology overflow');
    await back.focus();await page.keyboard.press('Enter');await page.waitForURL(base);
    results.push({width,...layout});
  }
  return results;
}
