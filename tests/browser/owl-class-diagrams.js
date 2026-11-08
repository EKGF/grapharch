async (page) => {
  const base = page.url().split('#')[0];
  const figures = page.locator('figure.owl-class-diagram');
  const count = await figures.count();
  if (!count) throw new Error('No generated class diagrams');
  const results = [];
  for (const width of [1280, 390]) {
    await page.setViewportSize({width, height: 900});
    await page.goto(base);
    const dimensions = await figures.evaluateAll(nodes => nodes.map(figure => {
      const svg = figure.querySelector('svg');
      const region = figure.querySelector('[role="region"]');
      const texts = [...svg.querySelectorAll('text')];
      return {
        title: svg.getAttribute('aria-label'),
        width: svg.getBoundingClientRect().width,
        available: region.clientWidth,
        smallestText: Math.min(...texts.map(text => text.getBoundingClientRect().height)),
        links: [...svg.querySelectorAll('a[href]')].map(link => link.getAttribute('href')),
      };
    }));
    for (const drawing of dimensions) {
      if (!drawing.title || drawing.width > drawing.available + 1) throw new Error(`Clipped diagram: ${drawing.title}`);
      if (drawing.smallestText < 8) throw new Error(`Unreadable labels at ${width}px: ${drawing.title} (${drawing.smallestText})`);
      for (const link of drawing.links) {
        if (link.startsWith('#') && !(await page.locator(`[id="${link.slice(1)}"]`).count())) throw new Error(`Broken class link: ${link}`);
      }
    }
    results.push({viewport: width, diagrams: dimensions.length, minimumTextHeight: Math.min(...dimensions.map(item => item.smallestText))});
  }
  for (const scheme of ['dark', 'light']) {
    await page.emulateMedia({colorScheme: scheme});
    const toggle = page.locator('.color-scheme-toggle');
    if (await toggle.count()) {
      const current = await page.locator('html').getAttribute('data-color-scheme');
      if (current !== scheme) await toggle.click();
    }
    const canvas = await figures.first().locator('[role="region"]').evaluate(node => getComputedStyle(node).backgroundColor);
    if (canvas !== 'rgb(255, 255, 255)') throw new Error(`Diagram ink has no controlled light canvas in ${scheme} mode`);
  }
  const summary = figures.first().locator('summary');
  await summary.focus();
  await page.keyboard.press('Enter');
  if (!(await figures.first().locator('details').evaluate(node => node.open))) throw new Error('Relationship text is not keyboard accessible');
  const local = page.locator('figure.owl-class-diagram svg a[href^="#"]').first();
  if (await local.count()) {
    const href = await local.getAttribute('href');
    await local.focus();
    await page.keyboard.press('Enter');
    if (new URL(page.url()).hash !== href) throw new Error('Class link keyboard navigation failed');
  }
  return {figures: count, layouts: results, keyboardTextAlternative: true, links: true, themes: ["light", "dark"]};
}
