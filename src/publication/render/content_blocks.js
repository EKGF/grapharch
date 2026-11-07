(() => {
  if (window.grapharchContentBlocks) return;
  window.grapharchContentBlocks = true;
  function initialize() {
    document.querySelectorAll('.ga-tabs').forEach((group, groupIndex) => {
      const panels = Array.from(group.children).filter(node => node.classList.contains('ga-tab'));
      if (!panels.length) return;
      const list = document.createElement('div');
      list.className = 'ga-tab-list';
      list.setAttribute('role', 'tablist');
      list.setAttribute('aria-label', 'Content views');
      const buttons = panels.map((panel, index) => {
        const button = document.createElement('button');
        const base = `ga-content-${groupIndex}-${index}`;
        button.type = 'button';
        button.id = `${base}-tab`;
        button.textContent = panel.dataset.title;
        button.setAttribute('role', 'tab');
        button.setAttribute('aria-controls', `${base}-panel`);
        panel.id = `${base}-panel`;
        panel.setAttribute('role', 'tabpanel');
        panel.setAttribute('aria-labelledby', button.id);
        panel.tabIndex = 0;
        panel.querySelector(':scope > .ga-tab-title').hidden = true;
        list.append(button);
        return button;
      });
      function select(index, focus) {
        panels.forEach((panel, i) => {
          panel.hidden = i !== index;
          buttons[i].setAttribute('aria-selected', String(i === index));
          buttons[i].tabIndex = i === index ? 0 : -1;
        });
        if (focus) buttons[index].focus();
      }
      buttons.forEach((button, index) => {
        button.addEventListener('click', () => select(index, false));
        button.addEventListener('keydown', event => {
          const next = event.key === 'ArrowRight' ? (index + 1) % buttons.length
            : event.key === 'ArrowLeft' ? (index + buttons.length - 1) % buttons.length
            : event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : null;
          if (next !== null) { event.preventDefault(); select(next, true); }
        });
      });
      function revealHash() {
        let target;
        try { target = document.getElementById(decodeURIComponent(location.hash.slice(1))); } catch { return; }
        if (!target) return;
        const index = panels.findIndex(panel => panel.contains(target));
        if (index >= 0) select(index, false);
      }
      group.prepend(list);
      select(0, false);
      revealHash();
      window.addEventListener('hashchange', revealHash);
    });
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initialize);
  else initialize();
})();
