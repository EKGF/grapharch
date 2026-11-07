(() => {
  const root = document.querySelector('[data-showcase-gallery]');
  if (!root) return;
  const form = root.querySelector('.showcase-discovery');
  const search = form.querySelector('input');
  const filters = [...form.querySelectorAll('select')];
  const cards = [...root.querySelectorAll('.showcase-card')];
  const state = root.querySelector('[data-gallery-count]');
  const empty = root.querySelector('[data-gallery-empty]');
  const key = select => `showcase-${select.dataset.galleryFilter}`;
  const terms = card => card.textContent.toLocaleLowerCase();
  const indexed = cards.map(card => ({card, text: terms(card)}));
  function readURL() {
    const params = new URL(location.href).searchParams;
    search.value = (params.get('showcase-q') || '').slice(0, 200);
    filters.forEach(select => {
      const value = params.get(key(select)) || '';
      select.value = [...select.options].some(option => option.value === value) ? value : '';
    });
  }
  function apply(writeURL) {
    const words = search.value.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
    let visible = 0;
    indexed.forEach(({card, text}) => {
      const matches = words.every(word => text.includes(word)) && filters.every(select => !select.value || (card.dataset[select.dataset.galleryFilter] || '').split('|').includes(select.value));
      card.hidden = !matches;
      if (matches) visible++;
    });
    state.textContent = `${visible} of ${cards.length} showcases`;
    empty.hidden = visible !== 0;
    if (writeURL) {
      const url = new URL(location.href);
      const update = (name, value) => value ? url.searchParams.set(name, value) : url.searchParams.delete(name);
      update('showcase-q', search.value.trim());
      filters.forEach(select => update(key(select), select.value));
      history.replaceState(null, '', url);
    }
  }
  form.hidden = false;
  readURL(); apply(false);
  form.addEventListener('submit', event => event.preventDefault());
  search.addEventListener('input', () => apply(true));
  filters.forEach(select => select.addEventListener('change', () => apply(true)));
  form.addEventListener('reset', event => {
    event.preventDefault(); search.value = ''; filters.forEach(select => select.value = ''); apply(true); search.focus();
  });
  window.addEventListener('popstate', () => { readURL(); apply(false); });
})();
