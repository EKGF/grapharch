async (page) => {
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.reload();
    await page.locator('[data-toc-expand]').click();
    const levels = await page.locator('#toc > ol').evaluate(root => {
      const visit = (list, depth) => [...list.children].flatMap(li => {
        const link = li.querySelector(':scope > a');
        const label = link?.querySelector('.toc-label');
        if (!label) return [];
        const range = document.createRange();
        range.selectNodeContents(label);
        const first = range.getClientRects()[0];
        const nested = li.querySelector(':scope > ol');
        return [{ depth, x: first.x, text: label.textContent }, ...(nested ? visit(nested, depth + 1) : [])];
      });
      return visit(root, 0);
    });
    const depthPositions = new Map();
    for (const row of levels) {
      if (!depthPositions.has(row.depth)) depthPositions.set(row.depth, row.x);
      check(Math.abs(row.x - depthPositions.get(row.depth)) < 1, `Sibling labels differ at ${width}px: ${JSON.stringify(row)}`);
    }
    const positions = [...depthPositions].sort((a, b) => a[0] - b[0]);
    const steps = positions.slice(1).map((row, index) => row[1] - positions[index][1]);
    check(steps.every(step => step > 8 && step < 25), `Each depth needs one compact indent at ${width}px; found ${steps}`);
    check(steps.every(step => Math.abs(step - steps[0]) < 1), `Depth indentation must be uniform at ${width}px; found ${steps}`);
    check(await page.locator('#toc').evaluate(node => node.scrollWidth <= node.clientWidth), `Sidebar must not overflow at ${width}px`);
  }
  return { siblingAlignment: true, uniformDepth: true, desktop: true, mobile: true };
}
