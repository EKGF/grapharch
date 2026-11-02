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
  const apply = () => {
    const dark = (choice ?? (system.matches ? 'dark' : 'light')) === 'dark';
    root.dataset.colorScheme = dark ? 'dark' : 'light';
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
