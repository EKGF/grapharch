async (page) => {
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  await page.setViewportSize({ width: 1440, height: 1000 });
  const banner = page.locator('.authority-banner');
  const original = await banner.textContent();
  const labels = ["EKGF Editor's Draft", 'W3C Recommendation', 'EDM Association Documentation Preview'];
  const positions = [];
  for (const label of labels) {
    await banner.evaluate((element, value) => { element.textContent = value; }, label);
    const geometry = await banner.evaluate(element => {
      const box = element.getBoundingClientRect();
      const range = document.createRange();
      range.selectNodeContents(element);
      const text = range.getBoundingClientRect();
      return { top: text.top - box.top, width: box.width, height: box.height, textBottom: text.bottom, bottom: box.bottom };
    });
    check(geometry.top >= 20 && geometry.top <= 30, `Ribbon label must start at the top padding: ${label}: ${JSON.stringify(geometry)}`);
    check(geometry.width === 26, 'Ribbon width changed');
    check(geometry.textBottom <= geometry.bottom, 'Ribbon text is clipped');
    positions.push(geometry.top);
  }
  check(Math.max(...positions) - Math.min(...positions) < 1, 'Ribbon anchor depends on label length');
  await banner.evaluate((element, value) => { element.textContent = value; }, original);
  await page.screenshot({ path: 'output/ribbon-review/desktop.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  const mobile = await banner.evaluate(element => {
    const style = getComputedStyle(element);
    const box = element.getBoundingClientRect();
    return { mode: style.writingMode, height: box.height, width: box.width };
  });
  check(mobile.mode === 'horizontal-tb' && mobile.height < 50, 'Mobile badge must stay compact and horizontal');
  check(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Mobile overflow');
  await page.screenshot({ path: 'output/ribbon-review/mobile.png' });
  return { passed: true, labelTop: positions, mobile };
}
