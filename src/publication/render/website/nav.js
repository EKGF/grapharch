(() => {
  "use strict";
  const sidebar = document.querySelector('.site-navigation');
  const navigation = sidebar.querySelector('nav');
  const browse = sidebar.querySelector(':scope > details');
  const branches = [...navigation.querySelectorAll('details[data-id]')];
  const key = 'grapharch:navigation:v1:' + JSON.stringify([sidebar.dataset.project, sidebar.dataset.target]);
  const breakpoint = matchMedia('(max-width:760px)');
  const mobile = () => breakpoint.matches;
  let mobileOpen = true;
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
