(() => {
  "use strict";
  const sidebar = document.querySelector('.site-navigation');
  // Project landing pages intentionally have no documentation rail.
  if (!sidebar) return;
  const navigation = sidebar.querySelector('nav');
  const browse = sidebar.querySelector(':scope > details');
  const branches = [...navigation.querySelectorAll('details[data-id]')];
  const key = 'grapharch:navigation:v1:' + JSON.stringify([sidebar.dataset.project, sidebar.dataset.target]);
  const breakpoint = matchMedia('(max-width:760px)');
  const mobile = () => breakpoint.matches;
  let mobileOpen = !sidebar.closest('.gallery-layout');
  const restore = () => {
    const saved = JSON.parse(sessionStorage.getItem(key));
    if (saved === null) return;
    mobileOpen = saved.mobileOpen;
    for (const branch of branches) {
      if (Object.hasOwn(saved.branches, branch.dataset.id)) branch.open = saved.branches[branch.dataset.id];
    }
    browse.open = !mobile() || mobileOpen;
    sidebar.scrollTop = saved.desktopScroll;
    navigation.scrollTop = saved.mobileScroll;
  };
  restore();
  browse.open = !mobile() || mobileOpen;
  const save = () => {
    if (mobile()) mobileOpen = browse.open;
    sessionStorage.setItem(key, JSON.stringify({
      branches: Object.fromEntries(branches.map(branch => [branch.dataset.id, branch.open])),
      mobileOpen,
      desktopScroll: sidebar.scrollTop,
      mobileScroll: navigation.scrollTop,
    }));
  };
  // Snapshot every branch, including closed ones, before the next page replaces
  // this document. Identity follows the graph subject, not a label or route.
  for (const branch of branches) branch.addEventListener('toggle', save);
  browse.addEventListener('toggle', save);
  breakpoint.addEventListener('change', () => { browse.open = !mobile() || mobileOpen; });
  sidebar.addEventListener('scroll', save, { passive: true });
  navigation.addEventListener('scroll', save, { passive: true });
  navigation.addEventListener('click', event => {
    if (event.target.closest('a')) save();
  });
  addEventListener('pagehide', save);
  addEventListener('pageshow', event => { if (event.persisted) restore(); });
  save();
})();

(() => {
  const menu = document.querySelector('.community-menu');
  if (!menu) return;
  const compact = matchMedia('(max-width:1150px)');
  const resize = () => { menu.open = !compact.matches; };
  resize();
  compact.addEventListener('change', resize);
  menu.addEventListener('keydown', event => {
    if (event.key === 'Escape' && compact.matches) {
      menu.open = false;
      menu.querySelector('summary').focus();
    }
  });
})();

(() => {
  // Primary sections scroll sideways when they do not fit; keep the current
  // section in view and fade the trailing edge while more sections follow.
  const tabs = document.querySelector('.documentation-product-row .primary-navigation');
  if (!tabs) return;
  const update = () => {
    const more = tabs.scrollWidth - tabs.clientWidth - tabs.scrollLeft > 1;
    if (more) tabs.dataset.overflow = 'more';
    else delete tabs.dataset.overflow;
  };
  const active = tabs.querySelector('.active-section');
  if (active && tabs.scrollWidth > tabs.clientWidth) {
    const left = active.offsetLeft - tabs.offsetLeft;
    const right = left + active.offsetWidth;
    if (left < tabs.scrollLeft || right > tabs.scrollLeft + tabs.clientWidth) {
      tabs.scrollLeft = Math.max(0, right - tabs.clientWidth + 16);
    }
  }
  update();
  tabs.addEventListener('scroll', update, { passive: true });
  addEventListener('resize', update, { passive: true });
})();
