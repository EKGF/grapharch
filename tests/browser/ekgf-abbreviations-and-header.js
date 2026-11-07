async (page) => {
  const reports = [];
  for (const site of ['principles', 'method', 'catalog', 'maturity']) {
    const path = site === 'catalog' ? 'catalog/use-case/client-360/' : `${site}/`;
    await page.goto(`http://localhost:8850/${path}`);
    await page.evaluate(() => localStorage.setItem('grapharch:color-scheme', 'light'));
    await page.reload();
    await page.evaluate(() => document.fonts.ready);
    const report = await page.evaluate(() => {
      const article = document.querySelector('article');
      const abbreviations = [...article.querySelectorAll('abbr')];
      if (!abbreviations.length) throw new Error('Source-owned abbreviation meanings missing');
      for (const term of abbreviations) {
        if (!term.title.trim()) throw new Error('Abbreviation meaning is empty');
        if (term.textContent === 'EKG' && /^[Fs]/.test(term.nextSibling?.textContent ?? '')) throw new Error('EKG incorrectly matched inside EKGF or EKGs');
      }
      if (article.querySelector('code abbr, pre abbr')) throw new Error('Abbreviation markup altered literal code');
      const identity = document.querySelector('.ekgf-identity');
      const logo = document.querySelector('.omg-identity img');
      if (!identity || !logo || !logo.complete || !logo.naturalWidth) throw new Error('Shared community identity missing');
      const header = document.querySelector('.community-header-row');
      const style = getComputedStyle(document.querySelector('.site-header'));
      const body = getComputedStyle(document.body);
      const links = [...document.querySelectorAll('.ekgf-header-navigation > a')].slice(0, 5).map(link => [link.textContent, link.href]);
      return {
        title: document.title,
        terms: abbreviations.slice(0, 8).map(term => [term.textContent, term.title]),
        literalCodeElements: article.querySelectorAll('code').length,
        header: { identity: identity.innerHTML, identityHref: identity.href, links, height: header.getBoundingClientRect().height, background: style.backgroundColor, color: style.color, font: body.fontFamily, fontSize: body.fontSize },
      };
    });
    await page.screenshot({path: `output/browser-review/${site}-abbreviations.png`});
    await page.locator('.site-search-open').click();
    await page.locator('#site-search-query').fill('EKG');
    const result = page.locator('.site-search-results a').first();
    await result.waitFor();
    const destination = await result.getAttribute('href');
    if (!new URL(destination, page.url()).pathname.startsWith(`/${site}/`)) throw new Error(`Search left the ${site} publication`);
    report.search = destination;
    await page.locator('.site-search-close').click();
    reports.push({site, ...report});
  }
  const common = JSON.stringify(reports[0].header);
  for (const report of reports) {
    if (JSON.stringify(report.header) !== common) throw new Error(`Shared header differs for ${report.site}: ${JSON.stringify(report.header)}`);
  }
  await page.goto('http://localhost:8850/principles/principle/01-identity/');
  const boundaries = await page.evaluate(() => {
    const article = document.querySelector('article');
    const terms = [...article.querySelectorAll('abbr')];
    const forum = terms.find(term => term.textContent === 'EKGF');
    if (!forum?.title.includes('Enterprise Knowledge Graph Forum')) throw new Error('EKGF must retain its own complete meaning');
    if (!/\bEKGs\b/.test(article.innerText) || terms.some(term => term.textContent === 'EKGs')) throw new Error('Plural EKGs was altered');
    return {EKGF: forum.title, pluralEKGs: 'literal'};
  });
  await page.goto('http://localhost:8850/method/concept/concept/concept-vocabulary/');
  const code = await page.evaluate(() => {
    const article = document.querySelector('article');
    const count = article.querySelectorAll('code').length;
    if (!count || article.querySelector('code abbr, pre abbr')) throw new Error('Actual code examples must stay literal');
    if (![...article.querySelectorAll('code')].some(code => code.textContent.includes('ekg-concept:ConceptSet'))) throw new Error('Canonical identifier code was changed');
    return {literalCodeElements: count};
  });
  reports.push({site: 'lexical-boundaries-and-code', boundaries, code, header: reports[0].header});
  return reports.map(({header, ...report}) => ({...report, sharedHeader: {height: header.height, font: header.font, fontSize: header.fontSize, background: header.background, color: header.color}}));
}
