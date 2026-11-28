async (page) => {
  // Every section of a project page shares one content column: the hero,
  // prose, card, table, code and callout sections, the section navigation
  // and the community header all align to the same left and right edges, and
  // the column is wide enough for command-line help on a wide screen. Cards
  // and tables in a prose section follow the column rather than a half-width
  // grid cell, and clicking the page draws no outline round the main area.
  const home = page.url();
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  for (const [width, minimum] of [[1440, 1100], [2000, 1100], [390, 300]]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.goto(home);
    const layout = await page.evaluate(() => {
      const edges = element => {
        const box = element.getBoundingClientRect();
        const style = getComputedStyle(element);
        return {
          left: Math.round(box.left + parseFloat(style.paddingLeft) + parseFloat(style.borderLeftWidth)),
          right: Math.round(box.right - parseFloat(style.paddingRight) - parseFloat(style.borderRightWidth)),
        };
      };
      const boxes = [];
      for (const container of document.querySelectorAll(':is(.project-hero,.project-section)>.project-container')) {
        const outer = container.getBoundingClientRect();
        boxes.push({ name: container.parentElement.className, left: Math.round(outer.left), right: Math.round(outer.right) });
      }
      const navigation = document.querySelector('.project-navigation>ul');
      if (navigation) boxes.push({ name: 'section navigation', ...edges(navigation) });
      const header = document.querySelector('.project-website .community-header-row');
      if (header) boxes.push({ name: 'header', ...edges(header) });
      const footer = document.querySelector('.project-website .site-footer-inner');
      if (footer) boxes.push({ name: 'footer', ...edges(footer) });
      const halves = [];
      for (const container of document.querySelectorAll('.project-section>.project-container')) {
        const cards = container.querySelectorAll(':scope>.project-card').length;
        for (const child of container.querySelectorAll(':scope>.table-scroll,:scope>table,:scope>pre,:scope>.project-card')) {
          if (child.classList.contains('project-card') && cards > 1) continue;
          // A boxed (panel or callout) section insets its content by its padding.
          const inner = edges(container);
          if (child.getBoundingClientRect().width < inner.right - inner.left - 2) {
            halves.push(`${container.parentElement.className}: ${child.className || child.tagName}`);
          }
        }
      }
      return { boxes, halves, overflow: document.documentElement.scrollWidth > innerWidth };
    });
    check(layout.boxes.length > 2, `No sections at ${width}`);
    const [first] = layout.boxes;
    for (const box of layout.boxes) {
      check(Math.abs(box.left - first.left) <= 1 && Math.abs(box.right - first.right) <= 1,
        `${box.name} spans ${box.left}-${box.right}, not ${first.left}-${first.right}, at ${width}`);
    }
    check(first.right - first.left >= Math.min(minimum, width - 48), `Content column is ${first.right - first.left}px at ${width}`);
    check(layout.halves.length === 0, `Narrower than the column at ${width}: ${layout.halves.join(', ')}`);
    check(!layout.overflow, `Page overflows at ${width}`);
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(home);
  // Click text that is not a control, as a reader selecting prose does.
  await page.locator('main :is(h1,h2,p):not(:has(a))').first().click();
  const outline = await page.evaluate(() => {
    const active = document.activeElement;
    const style = getComputedStyle(active);
    return active.matches('a,button,input,summary,select,textarea') ? 'none' : style.outlineStyle;
  });
  check(outline === 'none', `Clicking the page outlines ${outline}`);
  return 'ok';
}
