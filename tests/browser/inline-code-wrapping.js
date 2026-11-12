async (page) => {
  // Inline code in prose is one token: a token that fits on its line must
  // never break inside, for example between the `?` and the name of
  // `?customer`. Table cells are excluded: their columns shrink to fit the
  // viewport, so a token in a narrow column may wrap.
  const home = page.url();
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  for (const width of [1440, 390, 320]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.goto(home);
    await page.evaluate(() => document.fonts.ready);
    const result = await page.evaluate(() => {
      const broken = [];
      let checked = 0;
      for (const code of document.querySelectorAll(':not(pre)>code')) {
        const line = code.parentElement.getBoundingClientRect().width;
        if (!line || !code.getClientRects().length || code.closest('td,th')) continue;
        // A token wider than its line has to wrap; measure it unwrapped.
        const probe = code.cloneNode(true);
        probe.style.cssText = 'position:absolute;visibility:hidden;white-space:nowrap;display:inline;max-inline-size:none';
        code.parentElement.append(probe);
        const natural = probe.getBoundingClientRect().width;
        probe.remove();
        if (natural > line) continue;
        checked += 1;
        // Line boxes of the text itself, also inside an inline block.
        const range = document.createRange();
        range.selectNodeContents(code);
        const rows = new Set([...range.getClientRects()].filter(rect => rect.width > 0).map(rect => Math.round(rect.bottom)));
        if (rows.size > 1) broken.push(code.textContent);
      }
      return {
        checked,
        broken,
        overflow: document.documentElement.scrollWidth > innerWidth,
      };
    });
    check(result.checked > 0, `No inline code to check at ${width}`);
    check(result.broken.length === 0, `Inline code breaks inside at ${width}: ${result.broken.slice(0, 5).join(', ')}`);
    check(!result.overflow, `Page overflows at ${width}`);
  }
}
