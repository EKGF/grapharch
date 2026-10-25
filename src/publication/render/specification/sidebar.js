(() => {
  const handle = document.getElementById('toc-resizer');
  const toc = document.getElementById('toc');
  const root = document.documentElement;
  const desktop = matchMedia('(min-width: 1101px)');
  const key = 'grapharch:specification:toc-width';
  const minimum = 240;
  const maximum = () => Math.min(720, innerWidth - 560);
  let preferred = 432;
  let drag = null;

  // Storage can be prohibited by browser policy; resizing itself remains usable.
  try {
    const saved = localStorage.getItem(key);
    if (saved !== null && Number.isFinite(Number(saved))) preferred = Number(saved);
  } catch (error) {
    console.warn('GraphArch cannot read the saved sidebar width.', error);
  }
  const persist = () => {
    try { localStorage.setItem(key, String(preferred)); }
    catch (error) { console.warn('GraphArch cannot save the sidebar width.', error); }
  };
  const apply = (width) => {
    const bounded = Math.round(Math.max(minimum, Math.min(maximum(), width)));
    root.style.setProperty('--toc-width', `${bounded}px`);
    handle.setAttribute('aria-valuemin', String(minimum));
    handle.setAttribute('aria-valuemax', String(maximum()));
    handle.setAttribute('aria-valuenow', String(bounded));
    handle.setAttribute('aria-valuetext', `${bounded} pixels`);
    return bounded;
  };
  const refresh = () => {
    handle.hidden = !desktop.matches;
    if (desktop.matches) apply(preferred);
  };
  handle.addEventListener('pointerdown', (event) => {
    if (event.button !== 0 || !desktop.matches) return;
    event.preventDefault();
    handle.focus();
    drag = { id: event.pointerId, x: event.clientX, width: toc.getBoundingClientRect().width };
    handle.setPointerCapture(event.pointerId);
    root.classList.add('resizing-toc');
  });
  handle.addEventListener('pointermove', (event) => {
    if (drag && drag.id === event.pointerId) {
      preferred = apply(drag.width + event.clientX - drag.x);
    }
  });
  const finish = (event) => {
    if (!drag || drag.id !== event.pointerId) return;
    drag = null;
    root.classList.remove('resizing-toc');
    persist();
  };
  handle.addEventListener('pointerup', finish);
  handle.addEventListener('pointercancel', finish);
  handle.addEventListener('lostpointercapture', finish);
  handle.addEventListener('keydown', (event) => {
    if (!desktop.matches) return;
    const width = toc.getBoundingClientRect().width;
    const step = event.shiftKey ? 64 : 16;
    const widths = { ArrowLeft: width - step, ArrowRight: width + step, Home: minimum, End: maximum() };
    if (!(event.key in widths)) return;
    event.preventDefault();
    preferred = apply(widths[event.key]);
    persist();
  });
  handle.addEventListener('dblclick', () => {
    preferred = apply(432);
    persist();
  });
  addEventListener('resize', refresh);
  desktop.addEventListener('change', refresh);
  refresh();
})();
