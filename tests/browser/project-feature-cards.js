async (page) => {
  const home = page.url();
  const check = (value, message) => { if (!value) throw new Error(message); };
  for (const width of [1440, 1000, 390]) {
    for (const scheme of ['light', 'dark']) {
      await page.setViewportSize({width, height: 1100});
      await page.goto(home);
      await page.evaluate(() => document.fonts.ready);
      await page.evaluate(scheme => document.documentElement.dataset.colorScheme = scheme, scheme);
      const metrics = await page.locator('.project-section-features').evaluate(section => {
        const heading = section.querySelector('h2');
        const cards = [...section.querySelectorAll('.project-card')];
        return {
          centered: getComputedStyle(heading).textAlign,
          overflow: document.documentElement.scrollWidth > innerWidth,
          cards: cards.map(card => ({
            x: card.getBoundingClientRect().x,
            width: card.getBoundingClientRect().width,
            size: getComputedStyle(card.querySelector('h3')).fontSize,
            shadow: getComputedStyle(card).boxShadow,
            radius: getComputedStyle(card).borderRadius,
            icon: !!card.querySelector('.project-card-icon[aria-hidden="true"] svg'),
            tone: getComputedStyle(card.querySelector('.project-card-icon')).color,
            text: getComputedStyle(card.querySelector('p')).color,
          })),
        };
      });
      check(!metrics.overflow, `${width}/${scheme}: horizontal page overflow`);
      check(metrics.centered === 'center', 'Feature heading must be centered');
      check(metrics.cards.length >= 2, 'Fixture must contain at least two cards');
      for (const card of metrics.cards) {
        check(card.icon && card.size === '16px', 'Missing decorative icon or incorrect card title size');
        check(card.shadow !== 'none' && card.radius === '12px', 'Missing restrained card elevation');
        check(card.width < width, 'Card exceeds viewport');
      }
      check(metrics.cards[0].tone !== metrics.cards[1].tone, 'Explicit primary/accent icon tones should differ');
      if (width === 390) check(metrics.cards[0].x === metrics.cards[1].x, 'Mobile cards should stack');
    }
  }
  return 'Feature-card geometry, typography, icons and responsive light/dark presentation verified.';
}
