// Word-class colors in the Japanese text, per theme.

export const CLASSES = ['noun', 'verb', 'particle', 'adjective', 'auxiliary'];

export const CLASS_NAMES = {
  noun: 'subst.',
  verb: 'verbo',
  particle: 'partícula',
  adjective: 'adj./adv.',
  auxiliary: 'aux.',
};

export const DEFAULT_COLORS = {
  light: { noun: '#2F5FB3', verb: '#B2452E', particle: '#8A8780', adjective: '#2E7D5B', auxiliary: '#8E5BA8' },
  dark: { noun: '#7CC4E8', verb: '#F0937D', particle: '#9A9CA5', adjective: '#7FCBA4', auxiliary: '#C8A2E0' },
};

const HEX = /^#[0-9a-fA-F]{6}$/;

/** Saved colors completed with the defaults; invalid or unknown entries are dropped. */
export function normalizeColors(saved) {
  const out = {};
  for (const theme of ['light', 'dark']) {
    const mine = saved?.[theme] ?? {};
    out[theme] = Object.fromEntries(
      CLASSES.map((c) => [c, HEX.test(mine[c] ?? '') ? mine[c] : DEFAULT_COLORS[theme][c]]),
    );
  }
  return out;
}

/** Writes the colors as `--pos-<class>` per theme; `on = false` hides them. */
export function applyColors(saved, on) {
  const colors = normalizeColors(saved);
  let style = document.getElementById('pos-colors');
  if (!style) {
    style = document.createElement('style');
    style.id = 'pos-colors';
    document.head.append(style);
  }
  style.textContent = ['light', 'dark']
    .map((t) => `:root[data-theme='${t}'] {${CLASSES.map((c) => `--pos-${c}: ${colors[t][c]};`).join(' ')}}`)
    .join('\n');
  document.documentElement.classList.toggle('no-pos', !on);
}
