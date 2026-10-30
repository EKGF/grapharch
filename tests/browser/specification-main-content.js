async (page) => {
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  await page.setViewportSize({width:1440,height:1000});
  await page.evaluate(()=>localStorage.clear());
  await page.reload();
  const intro = page.locator('main section').filter({has:page.locator('h2', {hasText:'Introduction'})}).first();
  const heading = intro.locator(':scope > h2');
  await heading.scrollIntoViewIfNeeded();
  const geometry = await page.evaluate(()=>{
    const main=document.querySelector('main').getBoundingClientRect();
    const toc=document.querySelector('#toc').getBoundingClientRect();
    return {gap:main.left-toc.right,tocWidth:toc.width};
  });
  assert(geometry.gap>=80 && geometry.gap<=96,`specification content gutter is ${geometry.gap}px; expected W3C-like breathing room`);
  const example = page.locator('#ex-comments');
  const marker = example.locator('.marker');
  assert((await marker.textContent()).replace(/\s+/g,' ').trim()==='Example 1: Use of comments in N-Triples','numbered example caption is missing or altered');
  assert(await marker.locator('a').evaluate(e=>getComputedStyle(e).opacity==='1'),'example number is hidden by heading-permalink styles');
  const box=await example.evaluate(e=>{const s=getComputedStyle(e);return {background:s.backgroundColor,border:s.borderLeftColor,width:parseFloat(s.borderLeftWidth)};});
  assert(box.background==='rgb(252, 250, 238)' && box.border==='rgb(224, 203, 82)' && box.width>=7,'example must have the reference pale-yellow box and gold accent');
  const permalink=heading.locator('a.self-link');
  const position=await permalink.evaluate(a=>{
    const h=a.parentElement;const text=[...h.childNodes].find(n=>n.nodeType===Node.TEXT_NODE && n.textContent.trim());
    const range=document.createRange();range.selectNodeContents(text);
    return {link:a.getBoundingClientRect().right,text:range.getBoundingClientRect().left,opacity:parseFloat(getComputedStyle(a).opacity)};
  });
  assert(position.link<=position.text && position.opacity>0,'heading permalink must be visible in the left gutter');
  const nested=page.locator('.header-wrapper > a.self-link').first();
  assert((await nested.evaluate(a=>getComputedStyle(a,'::before').content)).includes('§'),'native subheading has no section-sign permalink');
  const grammar=page.locator('a[href="#grammar-production-triple"] > code').first();
  assert(await grammar.evaluate(e=>getComputedStyle(e).color==='rgb(255, 69, 0)'),'inline grammar tokens should retain the reference orange-red color');
  assert(await example.locator('pre code').evaluate(e=>getComputedStyle(e).color!=='rgb(255, 69, 0)'),'block examples must not inherit inline-token coloring');
  await example.scrollIntoViewIfNeeded();
  await page.screenshot({path:'output/playwright/specification-main-content-desktop.png'});
  await page.setViewportSize({width:390,height:844});
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'main content overflows the mobile viewport');
  await page.screenshot({path:'output/playwright/specification-main-content-mobile.png'});
  return {gutter:geometry.gap,exampleCaption:true,exampleBox:true,headingPermalinks:true,inlineGrammarColor:true,mobile:true};
}
