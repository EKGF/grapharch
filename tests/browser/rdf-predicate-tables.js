async page => {
  const measurements = [];
  for (const width of [1280, 390]) {
    await page.setViewportSize({width, height: 900});
    const metrics = await page.evaluate(() => {
      const codes = [...document.querySelectorAll('.rdf-properties td:first-child>code')];
      return {
        overflow: document.documentElement.scrollWidth > innerWidth + 1,
        predicates: codes.filter(code => ['sh:targetClass', 'rdfs:isDefinedBy'].includes(code.textContent)).map(code => ({name: code.textContent, height: code.getBoundingClientRect().height, line: parseFloat(getComputedStyle(code).lineHeight)})),
        long: codes.filter(code => code.textContent.includes('long-predicate-')).map(code => ({width: code.getBoundingClientRect().width, height: code.getBoundingClientRect().height}))
      };
    });
    if (metrics.overflow) throw new Error(`RDF table page overflow at ${width}px`);
    if (metrics.predicates.length !== 2 || metrics.predicates.some(predicate => predicate.height > predicate.line * 1.2)) throw new Error(`Short RDF predicate wrapped: ${JSON.stringify(metrics)}`);
    if (metrics.long.length !== 1 || metrics.long[0].width > width * .35 + 1) throw new Error('Long predicate must wrap inside its bounded column');
    measurements.push({width, ...metrics});
  }
  return measurements;
}
