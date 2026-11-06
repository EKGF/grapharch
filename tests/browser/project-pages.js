async (page) => {
  const base = page.url().replace(/\/(?:why|concepts|adopt|roadmap|spec-versions)\/$/, '/');
  const routes = ['', 'why/', 'concepts/', 'adopt/', 'roadmap/', 'spec-versions/'];
  for (const route of routes) {
    for (const width of [1440, 390]) {
      for (const scheme of ['light', 'dark']) {
        await page.setViewportSize({width, height: 1000});
        await page.goto(base + route);
        await page.waitForLoadState('load');
        await page.evaluate(() => document.fonts.ready);
        await page.evaluate(s => document.documentElement.dataset.colorScheme = s, scheme);
        const metrics = await page.evaluate(() => {
          const hero = document.querySelector('.project-hero');
          const size = el => getComputedStyle(el).fontSize;
          return {
            landing: hero.classList.contains('project-hero-landing'),
            heroSize: size(hero.querySelector('h1')),
            header: !!document.querySelector('.community-header-row'),
            footer: !!document.querySelector('.ekgf-footer'),
            overflow: document.documentElement.scrollWidth > innerWidth,
            sections: [...document.querySelectorAll('.project-section')].map(section => ({
              width: section.querySelector('.project-container').getBoundingClientRect().width,
              limit: section.classList.contains('project-section-resources') || !section.querySelector('.project-card') ? 896 : 1024,
            })),
            cards: [...document.querySelectorAll('.project-card')].map(card => ({
              title: size(card.querySelector('h3')),
              resource: !!card.closest('.project-section-resources'),
              padding: getComputedStyle(card).paddingLeft,
              body: getComputedStyle(card.querySelector('p')).color,
              muted: getComputedStyle(card).getPropertyValue('--muted').trim(),
            })),
            actions: [...document.querySelectorAll('.project-card-action a')].map(a => ({weight: getComputedStyle(a).fontWeight, icon: !!a.querySelector('svg[aria-hidden="true"]')})),
          };
        });
        const label = `${route || 'home'} ${width}/${scheme}`;
        const expectedSize = route ? (width === 390 ? '36px' : '60px') : (width === 390 ? '48px' : '72px');
        if (metrics.landing !== !route || metrics.heroSize !== expectedSize) throw Error(`${label}: incorrect hero role or typography ${JSON.stringify(metrics)}`);
        if (!metrics.header || !metrics.footer || metrics.overflow) throw Error(`${label}: missing shared chrome or page overflow`);
        for (const section of metrics.sections) if (section.width > section.limit) throw Error(`${label}: content measure exceeds ${section.limit}px`);
        for (const card of metrics.cards) {
          if (card.title !== (card.resource ? '20px' : '16px') || card.padding !== '24px') throw Error(`${label}: inconsistent card typography/inset ${JSON.stringify(card)}`);
        }
        if (metrics.actions.some(a => a.weight !== '500' || !a.icon)) throw Error(`${label}: inconsistent card CTA`);
      }
    }
  }
  return 'All six project pages share typography, measures, chrome, card insets and CTA styling across desktop/mobile and light/dark.';
}
