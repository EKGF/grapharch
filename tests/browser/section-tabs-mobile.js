// Run against a sectioned documentation website whose primary sections do not
// fit at phone width (for example an imported MkDocs site with five or more
// top-level sections). Pass the route of a page in the LAST section as
// `lastSection`, e.g. `/terms/`.
async (page, lastSection = '/terms/') => {
  const base = await page.locator('.documentation-product-row a').first().evaluate(node => node.href);
  const route = path => new URL(path.replace(/^\//, ''), base).href;
  const navigator = page.locator('.documentation-product-row .primary-navigation');
  for (const width of [390, 1440]) {
    await page.setViewportSize({width, height: 844});
    await page.goto(route('/'));
    if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1)) throw new Error(`Page scrolls horizontally at ${width}px`);
    const state = await navigator.evaluate(node => {
      const style = getComputedStyle(node);
      const tab = node.querySelector('li');
      return {
        overflows: node.scrollWidth > node.clientWidth + 1,
        scrolls: ['auto', 'scroll'].includes(style.overflowX),
        snap: style.scrollSnapType,
        align: tab ? getComputedStyle(tab).scrollSnapAlign : '',
        hint: node.dataset.overflow ?? '',
      };
    });
    if (width === 390) {
      if (!state.overflows) throw new Error('Fixture sections fit at 390px; choose a site with more sections');
      if (!state.scrolls) throw new Error('Overflowing sections are not scrollable');
      if (!state.snap.startsWith('x')) throw new Error('Section tabs do not snap horizontally');
      if (!state.align.includes('start')) throw new Error('Section tabs lack a snap position');
      if (state.hint !== 'more') throw new Error('No affordance shows that more sections follow');
    } else if (state.hint !== '') {
      throw new Error('Desktop sections show an overflow affordance');
    }
  }
  await page.setViewportSize({width: 390, height: 844});
  await page.goto(route(lastSection));
  const visible = await navigator.evaluate(node => {
    const active = node.querySelector('.active-section');
    if (!active) return null;
    const outer = node.getBoundingClientRect();
    const inner = active.getBoundingClientRect();
    return inner.left >= outer.left - 1 && inner.right <= outer.right + 1;
  });
  if (visible !== true) throw new Error('The current section tab is scrolled out of view');
  const hint = await navigator.evaluate(node => node.dataset.overflow ?? '');
  if (hint === 'more') throw new Error('Affordance remains at the end of the sections');
  const last = page.locator('.documentation-product-row .primary-navigation > ul > li > a').last();
  await last.focus();
  if (await last.evaluate(n => getComputedStyle(n).outlineStyle === 'none')) throw new Error('Section keyboard focus invisible');
  return {snap: true, affordance: true, activeVisible: true, keyboard: true};
}
