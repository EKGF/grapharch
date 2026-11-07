(() => {
  'use strict';
  const key = 'grapharch:color-scheme';
  const system = matchMedia('(prefers-color-scheme: dark)');
  const root = document.documentElement;
  let choice = localStorage.getItem(key);
  const validate = value => {
    if (value !== null && value !== 'light' && value !== 'dark') {
      throw new Error('Unsupported saved GraphArch color scheme');
    }
  };
  validate(choice);
  // Object documents do not consistently inherit an explicit embedding theme.
  // Preserve each authored query and invert only its color-scheme atoms when
  // the publication choice differs from the embedded document's native theme.
  const originalQueries = new WeakMap();
  const syncDiagram = object => {
    const svg = object.contentDocument;
    if (!svg || svg.documentElement.namespaceURI !== 'http://www.w3.org/2000/svg') return;
    const nativeDark = object.contentWindow.matchMedia('(prefers-color-scheme: dark)').matches;
    const invert = (root.dataset.colorScheme === 'dark') !== nativeDark;
    const walk = rules => {
      for (const rule of rules) {
        if (rule.type === CSSRule.MEDIA_RULE) {
          const original = originalQueries.get(rule) ?? rule.media.mediaText;
          originalQueries.set(rule, original);
          rule.media.mediaText = invert
            ? original.replace(/(\(\s*prefers-color-scheme\s*:\s*)(dark|light)(\s*\))/gi,
              (_, prefix, value, suffix) => `${prefix}${value.toLowerCase() === 'dark' ? 'light' : 'dark'}${suffix}`)
            : original;
        }
        if (rule.cssRules) walk(rule.cssRules);
      }
    };
    for (const sheet of svg.styleSheets) walk(sheet.cssRules);
  };
  const syncDiagrams = () => {
    for (const object of document.querySelectorAll('object.interactive-svg')) syncDiagram(object);
  };
  document.addEventListener('load', event => {
    if (event.target instanceof HTMLObjectElement && event.target.matches('.interactive-svg')) syncDiagram(event.target);
  }, true);
  new MutationObserver(syncDiagrams).observe(root, { attributes: true, attributeFilter: ['data-color-scheme'] });
  const apply = () => {
    const dark = (choice ?? (system.matches ? 'dark' : 'light')) === 'dark';
    root.dataset.colorScheme = dark ? 'dark' : 'light';
    syncDiagrams();
    const button = document.querySelector('.color-scheme-toggle');
    if (button) {
      button.setAttribute('aria-pressed', String(dark));
      button.title = dark ? 'Use light theme' : 'Use dark theme';
    }
  };
  // Apply before the stylesheet is encountered, avoiding a light-theme flash.
  apply();
  system.addEventListener('change', () => { if (choice === null) apply(); });
  addEventListener('storage', event => {
    if (event.key !== key && event.key !== null) return;
    choice = localStorage.getItem(key);
    validate(choice);
    apply();
  });
  addEventListener('DOMContentLoaded', () => {
    const button = document.querySelector('.color-scheme-toggle');
    if (!button) throw new Error('Community color scheme control is missing');
    apply();
    button.addEventListener('click', () => {
      choice = root.dataset.colorScheme === 'dark' ? 'light' : 'dark';
      localStorage.setItem(key, choice);
      apply();
    });
  }, { once: true });
})();
