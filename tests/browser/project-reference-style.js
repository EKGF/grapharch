async (page) => {
  const home = page.url();
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  for (const width of [1920, 1440, 1000, 390]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.goto(home);
    await page.evaluate(() => document.fonts.ready);
    const metrics = await page.evaluate(() => {
      const hero = document.querySelector('.project-hero .project-container');
      const description = [...hero.querySelectorAll('p')].find(p => p.textContent.startsWith('An OMG standard'));
      const panel = document.querySelector('.project-section-panel>.project-container');
      return {
        layoutWidth: document.body.getBoundingClientRect().width,
        headerWidth: document.querySelector('.community-header-row').getBoundingClientRect().width,
        heroWidth: hero.getBoundingClientRect().width,
        descriptionSize: parseFloat(getComputedStyle(description).fontSize),
        descriptionLine: parseFloat(getComputedStyle(description).lineHeight),
        descriptionHeight: description.getBoundingClientRect().height,
        panelWidth: panel?.getBoundingClientRect().width,
        panelPadding: panel ? parseFloat(getComputedStyle(panel).paddingLeft) : 0,
        panelBorder: panel ? parseFloat(getComputedStyle(panel).borderLeftWidth) : 0,
        panelRadius: panel ? parseFloat(getComputedStyle(panel).borderRadius) : 0,
        overflow: document.documentElement.scrollWidth > innerWidth,
      };
    });
    check(!metrics.overflow, `Project page overflows at ${width}`);
    check(metrics.panelBorder === 1 && metrics.panelRadius === 12, `Selected overview is not a bordered panel at ${width}`);
    check(metrics.panelWidth <= 896, `Overview panel is wider than the reference at ${width}`);
    if (width >= 1440) {
      check(metrics.headerWidth === Math.min(1440, metrics.layoutWidth), 'Community header lost the reference width');
      check(metrics.heroWidth === 896, 'Hero lost the reference content width');
      check(metrics.descriptionSize === 24 && metrics.descriptionLine === 32, 'Hero description lost the reference typography');
      check(metrics.descriptionHeight === 96, 'Reference description must wrap into three readable lines');
      check(metrics.panelPadding === 32, 'Overview panel lost its reference inset');
    }
  }
}
