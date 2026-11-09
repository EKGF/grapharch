async (page) => {
  const result = await page.evaluate(async () => {
    const source = [...document.scripts].map(s => s.textContent).find(s => s.includes('const addBranches'));
    if (!source) throw new Error('Specification sidebar script missing');
    const frame = document.createElement('iframe');
    document.body.append(frame);
    const doc = frame.contentDocument;
    doc.open();
    doc.write('<nav id="toc"><button data-toc-collapse>Collapse</button><button data-toc-expand>Expand</button><ol><li><a href="other.html#one">One</a><ol><li><a href="other.html#two">Two</a><ol><li><a href="other.html#three">Three</a><ol><li><a href="other.html#four">Four</a></li></ol></li></ol></li></ol></li></ol></nav>');
    doc.close();
    const script = doc.createElement('script');
    script.textContent = source;
    doc.body.append(script);
    const buttons = [...doc.querySelectorAll('.toc-toggle')];
    const defaults = buttons.map(b => b.getAttribute('aria-expanded'));
    if (defaults.join(',') !== 'true,true,false') throw new Error(`Expected first three heading levels visible, got ${defaults}`);
    doc.querySelector('[data-toc-expand]').click();
    if (buttons.some(b => b.getAttribute('aria-expanded') !== 'true')) throw new Error('Expand all failed');
    doc.querySelector('[data-toc-collapse]').click();
    if (buttons.some(b => b.getAttribute('aria-expanded') !== 'false')) throw new Error('Collapse all failed');
    buttons[0].click();
    if (buttons[0].getAttribute('aria-expanded') !== 'true') throw new Error('Individual toggle failed');
    frame.remove();
    return { defaults, expand: true, collapse: true, toggle: true };
  });
  return result;
}
