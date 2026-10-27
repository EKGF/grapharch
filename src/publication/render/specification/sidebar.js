(() => {
  const handle = document.getElementById('toc-resizer');
  const toc = document.getElementById('toc');
  if (!handle || !toc) return;
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

(() => {
  const toc = document.getElementById('toc');
  const dialog = document.getElementById('spec-search');
  const input = document.getElementById('spec-search-query');
  const kind = document.getElementById('spec-search-kind');
  const results = document.getElementById('spec-search-results');
  const status = document.getElementById('spec-search-status');
  const branches = [];
  const addBranches = (container) => { for (const li of container.querySelectorAll('li')) {
    const children = li.querySelector(':scope > ol');
    const link = li.querySelector(':scope > a');
    if (!children || !link || li.querySelector(':scope > .toc-toggle')) continue;
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'toc-toggle';
    button.setAttribute('aria-label', `Toggle ${link.textContent.trim()}`);
    button.setAttribute('aria-expanded', 'true');
    children.id = `toc-branch-${branches.length}`;
    button.setAttribute('aria-controls', children.id);
    button.textContent = '›';
    li.insertBefore(button, link);
    const set = (expanded) => {
      children.hidden = !expanded;
      button.setAttribute('aria-expanded', String(expanded));
    };
    button.addEventListener('click', () => set(children.hidden));
    branches.push({li, children, set});
    if (li.closest('.family-navigation') || li.parentElement.closest('li')) set(false);
  }};
  addBranches(toc);
  const reveal = () => {
    const candidates = [...toc.querySelectorAll('a[href]')].filter(a => a.pathname === location.pathname);
    const current = candidates.find(a => location.hash && a.hash === location.hash) || candidates.find(a => !a.hash);
    toc.querySelectorAll('[aria-current]').forEach(a => a.removeAttribute('aria-current'));
    if (!current) return;
    current.setAttribute('aria-current', location.hash ? 'location' : 'page');
    for (const branch of branches) if (branch.children.contains(current) || branch.li === current.closest('li')) branch.set(true);
  };
  toc.querySelector('[data-toc-collapse]')?.addEventListener('click', () => { branches.forEach(b => b.set(false)); });
  toc.querySelector('[data-toc-expand]')?.addEventListener('click', () => { branches.forEach(b => b.set(true)); });
  addEventListener('hashchange', reveal);
  reveal();
  const familyTree = toc.querySelector('[data-family-tree]');
  if (familyTree) {
    const disclosure = familyTree.closest('details');
    let started = false;
    disclosure.addEventListener('toggle', async () => {
      if (!disclosure.open || started) return;
      started = true; familyTree.textContent = 'Loading family navigation…';
      try {
        const response = await fetch(familyTree.dataset.navigationIndex);
        if (!response.ok) throw new Error(`Navigation index HTTP ${response.status}`);
        const pages = await response.json();
        if (!Array.isArray(pages) || pages.length > 8192) throw new Error('Invalid family navigation index');
        const root = document.createElement('ol');
        const lists = [];
        pages.forEach((p, i) => {
          if (typeof p.title !== 'string' || typeof p.href !== 'string' || !/^[a-z0-9-]+\.html$/.test(p.href) || (p.parent !== null && (!Number.isInteger(p.parent) || p.parent < 0 || p.parent >= i))) throw new Error('Invalid family navigation entry');
          const li = document.createElement('li');
          const a = document.createElement('a'); a.href = p.href; a.textContent = p.title;
          li.append(a);
          const children = document.createElement('ol'); lists.push({li,children});
          if (p.parent === null) root.append(li);
          else { const parent=lists[p.parent]; if (!parent.children.parentNode) parent.li.append(parent.children); parent.children.append(li); }
        });
        familyTree.replaceChildren(root); addBranches(familyTree); reveal();
      } catch (error) { familyTree.textContent = `Family navigation unavailable: ${error.message}`; }
    });
  }
  let entries;
  let loading;
  let opener;
  const load = async () => {
    if (entries) return;
    if (!loading) loading = (async () => {
      if (toc.dataset.searchIndex) {
        const response = await fetch(toc.dataset.searchIndex);
        if (!response.ok) throw new Error(`Search index HTTP ${response.status}`);
        entries = await response.json();
        if (!Array.isArray(entries) || entries.length > 262144 || entries.some(e => typeof e.title !== 'string' || typeof e.href !== 'string' || typeof e.kind !== 'string' || !/^(?:#[^\s]+|[a-z0-9-]+\.html(?:#[^\s]+)?)$/.test(e.href))) throw new Error('Invalid specification search index');
      } else {
        entries = [...document.querySelectorAll('main h2,main h3,main h4,main h5,main h6')].filter(h => !h.closest('nav,dialog,header')).map(h => {
          const section = h.closest('section');
          const id = h.id || (section && section.id);
          return {title:h.textContent.replace(/§$/, '').trim(), href:`#${id}`, kind:h.id ? 'section' : (section.dataset.searchKind || 'section'), context:''};
        }).filter(e => e.href !== '#null' && e.href !== '#');
      }
      entries = entries.map(e => ({...e, haystack:`${e.title} ${e.context || ''} ${e.identifier || ''}`.toLocaleLowerCase()}));
    })();
    await loading;
  };
  const filter = () => {
    if (!entries) return;
    const terms = input.value.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
    results.replaceChildren();
    let total = 0;
    for (const entry of entries) {
      if ((kind.value !== 'all' && entry.kind !== kind.value) || !terms.every(term => entry.haystack.includes(term))) continue;
      total++;
      if (total > 50) continue;
      const li = document.createElement('li');
      const a = document.createElement('a'); a.href = entry.href; a.textContent = entry.title;
      a.addEventListener('click', () => { opener = null; dialog.close(); });
      li.append(a);
      if (entry.context) { const context = document.createElement('small'); context.textContent = entry.context; li.append(context); }
      results.append(li);
    }
    status.textContent = `${total} results${total > 50 ? ' · showing the first 50; refine your search' : ''}`;
  };
  const open = async (source) => {
    opener = source; dialog.showModal(); input.focus(); status.textContent = 'Loading search index…';
    try { await load(); filter(); }
    catch (error) { status.textContent = `Search unavailable: ${error.message}`; }
  };
  toc.querySelector('[data-search-open]').addEventListener('click', event => { void open(event.currentTarget); });
  dialog.querySelector('[data-search-close]').addEventListener('click', () => dialog.close());
  dialog.addEventListener('close', () => { if (opener) opener.focus(); });
  input.addEventListener('input', filter); kind.addEventListener('change', filter);
  document.addEventListener('keydown', event => {
    // Search inputs consume Escape to clear their value before the native
    // dialog close action. Give this dialog one consistent Escape action.
    if (dialog.open && event.key === 'Escape') { event.preventDefault(); dialog.close(); return; }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault(); if (!dialog.open) void open(toc.querySelector('[data-search-open]'));
    }
  });
})();
