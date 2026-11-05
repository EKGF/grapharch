async (page) => {
  const home = page.url();
  for (const width of [1440, 1000, 390]) {
    for (const scheme of ['light', 'dark']) {
      await page.setViewportSize({width, height: 1100});
      await page.goto(home);
      await page.evaluate(() => document.fonts.ready);
      await page.evaluate(scheme => document.documentElement.dataset.colorScheme = scheme, scheme);
      const result = await page.locator('.project-section-resources').evaluate(section => {
        const cards = [...section.querySelectorAll('.project-card')];
        return {
          headingGap: cards[0].getBoundingClientRect().top - section.querySelector('h2').getBoundingClientRect().bottom,
          overflow: document.documentElement.scrollWidth > innerWidth,
          cards: cards.map(card => {
            const icon = card.querySelector('.project-card-icon');
            const text = card.querySelector('.project-card-content');
            return {x: card.getBoundingClientRect().x, width: card.getBoundingClientRect().width,
              icon: getComputedStyle(icon).width, tone: getComputedStyle(icon).color,
              beside: icon.getBoundingClientRect().right < text.getBoundingClientRect().left,
              title: getComputedStyle(text.querySelector('h3')).fontSize,
              bottom: getComputedStyle(text.lastElementChild).marginBottom,
              weight: getComputedStyle(text.querySelector('.project-card-action a')).fontWeight,
              cta: !!card.querySelector('.project-card-action a svg[aria-hidden="true"]')};
          }),
        };
      });
      if (result.headingGap !== 32 || result.overflow || result.cards.length < 2) throw new Error(`${width}/${scheme}: resource geometry mismatch (heading gap ${result.headingGap}px)`);
      const first = result.cards[0];
      for (const card of result.cards) {
        if (card.x !== first.x || card.width !== first.width || card.icon !== '24px' || !card.beside || card.title !== '20px' || card.bottom !== '0px' || card.weight !== '500' || !card.cta) {
          throw new Error(`${width}/${scheme}: resource-list composition mismatch ${JSON.stringify(card)}`);
        }
      }
      if (first.tone === result.cards[1].tone) throw new Error('Resource icon tones must remain distinct');
    }
  }
  return 'Single-column resource cards, horizontal icons, typography and CTAs verified at three widths in both themes.';
}
