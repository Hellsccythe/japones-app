// Light/dark theme: "system" follows the OS, "light"/"dark" force one.

export function resolveTheme(setting, prefersDark) {
  if (setting === 'light' || setting === 'dark') return setting;
  return prefersDark ? 'dark' : 'light';
}

let media;
let current = 'system';

/** Sets `data-theme` on <html>; with "system", follows OS changes live. */
export function applyTheme(setting) {
  current = setting;
  media ??= window.matchMedia('(prefers-color-scheme: dark)');
  media.onchange = () => current === 'system' && applyTheme('system');
  document.documentElement.dataset.theme = resolveTheme(setting, media.matches);
}

/** The theme actually shown ("light" or "dark"). */
export const shownTheme = () => document.documentElement.dataset.theme || 'dark';
