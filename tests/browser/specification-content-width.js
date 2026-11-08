async (page) => {
  // A deterministic wide relationship diagram and authored constraint table
  // exercise shared spec CSS independently of a particular ontology layout.
  await page.evaluate(() => {
    const section = document.createElement('section');
    section.id = 'responsive-content-regression';
    section.innerHTML = `<h2>Responsive class documentation</h2><p>${'A data product describes its contract, ownership, quality and access relationships. '.repeat(12)}</p><figure class="owl-class-diagram" style="width:fit-content;max-width:100%;margin:1.25rem auto"><div role="region" tabindex="0" aria-label="Class relationship diagram" style="overflow-x:auto;padding:12px"><svg xmlns="http://www.w3.org/2000/svg" width="1100" height="180" style="max-width:none"><rect x="1" y="1" width="1098" height="178" fill="white" stroke="black"/><text x="20" y="45" font-size="16">Data product — contract — ownership — quality — access</text></svg></div></figure><div class="table-scroll" tabindex="0"><table class="rdf-properties"><thead><tr><th>RDF predicate</th><th>Value</th></tr></thead><tbody><tr><td><code>rdfs:isDefinedBy</code></td><td>https://example.org/ontology/data-product-contracts-and-validation</td></tr><tr><td><code>sh:property</code></td><td>contractLifecycleStatus: skos:Concept; maximum count 1; optional authored lifecycle status for a DataContract.</td></tr></tbody></table></div><div class="example"><p>Example: data product metadata</p><pre><code>ex:product a dprod:DataProduct .</code></pre></div>`;
    document.querySelector('main').append(section);
  });
  const results = [];
  for (const width of [2048, 1280, 390]) {
    await page.setViewportSize({width, height:900});
    const dimensions = await page.locator('#responsive-content-regression').evaluate(section => {
      const rect = node => node.getBoundingClientRect();
      const main = rect(document.querySelector('main'));
      const region = section.querySelector('[role=region]');
      const table = rect(section.querySelector('table'));
      const example = section.querySelector('.example');
      const exampleStyle = getComputedStyle(example);
      const inset = parseFloat(exampleStyle.paddingLeft)+parseFloat(exampleStyle.paddingRight)+parseFloat(exampleStyle.borderLeftWidth)+parseFloat(exampleStyle.borderRightWidth);
      return {example:rect(example).width, code:rect(example.querySelector('pre')).width, inset, mainWidth:main.width, right:main.right, prose:rect(section.querySelector('p')).width, table:table.width, available:region.clientWidth, scroll:region.scrollWidth, text:rect(section.querySelector('text')).height, overflow:document.documentElement.scrollWidth > innerWidth+1};
    });
    if (dimensions.overflow) throw new Error(`Page overflows at ${width}`);
    if (Math.abs(dimensions.example-dimensions.mainWidth)>1 || Math.abs(dimensions.code+dimensions.inset-dimensions.example)>1) throw new Error('Example and code must share the content measure');
    if (dimensions.text < 15) throw new Error('Diagram text was shrunk');
    if (width === 2048) {
      if (Math.abs(dimensions.mainWidth-1200)>1) throw new Error(`Content must use the shared 75rem cap: ${JSON.stringify(dimensions)}`);
      if (dimensions.scroll > dimensions.available+1) throw new Error('Wide diagram scrolls despite available viewport space');
      if (dimensions.table < dimensions.mainWidth-2) throw new Error('Constraint table remains restricted to prose measure');
      if (Math.abs(dimensions.prose-dimensions.mainWidth)>1) throw new Error('Prose has a separate narrower cap');
    }
    if (width === 390) {
      const region=page.locator('#responsive-content-regression [role=region]');
      await region.focus();
      await page.keyboard.press('ArrowRight');
      await page.waitForFunction(()=>document.activeElement.scrollLeft>0);
    }
    await page.locator('#responsive-content-regression').scrollIntoViewIfNeeded();
    await page.screenshot({path:`output/playwright/specification-width-${width}.png`});
    results.push({width,...dimensions});
  }
  return results;
}
