async (page) => {
  const results=[];
  const actual=page.locator('figure.owl-class-diagram').filter({has:page.locator('figcaption').filter({hasText:/explicit date period/i})}).first();
  if(await actual.count()!==1)throw Error('Missing original oversized ExplicitDatePeriod fixture');
  await page.evaluate(()=>{
    const fixture=document.querySelector('figure.owl-class-diagram').cloneNode(true);
    fixture.id='oversized-diagram-regression';
    fixture.querySelector('.class-diagram-scroll').innerHTML='<svg xmlns="http://www.w3.org/2000/svg" width="2400" height="160" viewBox="0 0 2400 160" style="display:block;width:2400px;max-width:none;height:auto"><rect x="1" y="1" width="2398" height="158" fill="white" stroke="black"/><text x="20" y="70" font-size="18">Readable oversized diagram</text><text x="2180" y="70" font-size="18">Right edge</text></svg>';
    document.querySelector('main').append(fixture);
  });
  for(const viewport of [2048,1280,390]) {
    await page.setViewportSize({width:viewport,height:900});
    for(const figure of [actual,page.locator('#oversized-diagram-regression')]) {
      const region=figure.locator('.class-diagram-scroll');
      const measurements=await region.evaluate(r=>{const s=r.querySelector('svg');return {client:r.clientWidth,scroll:r.scrollWidth,svg:s.getBoundingClientRect().width,intrinsic:s.viewBox.baseVal.width,overflow:getComputedStyle(r).overflowX,track:getComputedStyle(r,'::-webkit-scrollbar').height,pageOverflow:document.documentElement.scrollWidth>innerWidth+1}});
      if(measurements.pageOverflow||measurements.scroll<=measurements.client||measurements.svg!==measurements.intrinsic)throw Error(JSON.stringify(measurements));
      if(measurements.track!=='12px')throw Error('Missing discoverable native scrollbar styling');
      await region.focus(); await page.keyboard.press('ArrowRight');
      await page.waitForFunction(()=>document.activeElement.scrollLeft>0);
      await region.evaluate(r=>r.scrollLeft=r.scrollWidth);
      if(!(await region.evaluate(r=>r.scrollLeft+r.clientWidth>=r.scrollWidth-1)))throw Error('Right edge remains clipped');
      await region.evaluate(r=>r.scrollLeft=0);
      results.push({viewport,...measurements});
    }
    if(viewport===390) {
      const region=page.locator('#oversized-diagram-regression .class-diagram-scroll');
      await region.scrollIntoViewIfNeeded();
      const box=await region.boundingBox(), session=await page.context().newCDPSession(page);
      await session.send('Emulation.setTouchEmulationEnabled',{enabled:true});
      const x=box.x+box.width-30,y=box.y+50;
      await session.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y}]});
      for(let offset=20;offset<=180;offset+=20)await session.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:x-offset,y}]});
      await session.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
      await page.waitForFunction(()=>document.querySelector('#oversized-diagram-regression .class-diagram-scroll').scrollLeft>0);
      await session.send('Emulation.setTouchEmulationEnabled',{enabled:false});await session.detach();
    }
    await actual.scrollIntoViewIfNeeded();
    await page.screenshot({path:`output/playwright/diagram-overflow-${viewport}.png`});
  }
  return results;
}
