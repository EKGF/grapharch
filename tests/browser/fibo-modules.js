async (page) => {
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  await page.setViewportSize({ width: 1440, height: 1000 });
  assert((await page.title()).includes('Financial Industry Business Ontology'), 'wrong publication');
  assert((await page.locator('.authority-banner').textContent()).includes('EDM Association Documentation Preview'), 'authority ribbon');
  const domain = page.locator('main > section > h3').filter({hasText:'foundations domain'});
  const module = page.locator('main > section > h4').filter({hasText:'parties module'});
  const ontology = page.locator('main > section > h5').filter({hasText:/^2\.8\.10\.1\. Parties Ontology/});
  assert(await domain.count() === 1, 'missing domain level');
  assert(await module.count() === 1, 'missing module level');
  assert(await ontology.count() === 1, 'missing ontology level');
  assert(await page.locator('h6[aria-level="7"]').count() > 100, 'deep term headings lost their semantic level');
  const toc = page.locator('.toc a').filter({hasText:'parties module'}).first();
  await toc.click();
  assert(page.url().includes('#'), 'module link did not navigate');
  const anchor = new URL(page.url()).hash.slice(1);
  assert(await page.locator(`[id="${anchor}"]`).count() === 1, 'broken module anchor');
  const pdf = await page.request.head('http://localhost:8872/specification/document.pdf');
  assert(pdf.ok() && pdf.headers()['content-type'].includes('application/pdf'), 'PDF download unavailable');
  await module.scrollIntoViewIfNeeded();
  await page.screenshot({path:'output/fibo-benchmark/desktop-module.png'});
  await page.setViewportSize({width:390,height:844});
  await module.scrollIntoViewIfNeeded();
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'mobile horizontal overflow');
  await page.screenshot({path:'output/fibo-benchmark/mobile-module.png'});
  return {passed:true, title:await page.title(), pdfBytes:pdf.headers()['content-length']};
}
