// Color picker for one word class: 8 suggestions, a free color field and "Restaurar padrão".
// The caller passes a store, so this module doesn't depend on the settings module.

import { escapeHtml } from './api.js';
import { icon } from './icons.js';
import { CLASS_NAMES, normalizeColors, suggestedColors, withColor, withoutColor, applyColors } from './colors.js';
import { shownTheme } from './theme.js';

let pop = null;
let saveTimer;
/** Colors chosen but not saved yet (the save waits a moment), so the next change builds on them. */
let pending = null;

function close() {
  pop?.remove();
  pop = null;
  document.removeEventListener('pointerdown', onOutside, true);
  document.removeEventListener('keydown', onKey, true);
}

function onOutside(e) {
  if (pop && !pop.contains(e.target)) close();
}

function onKey(e) {
  if (e.key === 'Escape') close();
}

/** Places the popover under `anchor`, kept inside the window. */
function place(anchor) {
  const r = anchor.getBoundingClientRect();
  const w = pop.offsetWidth;
  const h = pop.offsetHeight;
  const left = Math.min(Math.max(8, r.left), window.innerWidth - w - 8);
  const below = r.bottom + 6;
  const top = below + h > window.innerHeight - 8 ? Math.max(8, r.top - h - 6) : below;
  pop.style.left = left + 'px';
  pop.style.top = top + 'px';
}

/**
 * Opens the picker for `cls` next to `anchor`.
 * `store`: `{ get(): pos_colors, on(): pos_colors_on, save(next): Promise }`.
 * Colors are saved for the theme on screen only.
 */
export function openPalette(anchor, cls, store) {
  close();
  const theme = shownTheme();
  const colorOf = (saved) => normalizeColors(saved)[theme][cls];
  const saved = () => pending ?? store.get();
  const currentColor = () => colorOf(saved());
  const set = (next) => {
    applyColors(next, store.on());
    // The free color field fires on every drag step; save once it settles.
    pending = next;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      pending = null;
      Promise.resolve(store.save(next)).catch(() => {});
    }, 250);
    const c = colorOf(next);
    pop?.querySelectorAll('.swatch').forEach((b) => b.classList.toggle('on', b.dataset.color === c));
    const input = pop?.querySelector('input[type=color]');
    if (input) input.value = c.toLowerCase();
  };

  pop = document.createElement('div');
  pop.className = 'palette';
  pop.setAttribute('role', 'dialog');
  pop.setAttribute('aria-label', `Cor de ${CLASS_NAMES[cls]}`);
  pop.innerHTML = `
    <div class="palette-head"><strong>${escapeHtml(CLASS_NAMES[cls])}</strong><small>tema ${theme === 'dark' ? 'escuro' : 'claro'}</small>
      <span class="grow"></span><button type="button" class="btn-icon close" title="Fechar" aria-label="Fechar">${icon('x', 16)}</button></div>
    <div class="swatches">${suggestedColors(theme)
      .map(
        (c) =>
          `<button type="button" class="swatch${c === currentColor() ? ' on' : ''}" data-color="${c}" style="--c:${c}" title="${c}" aria-label="${c}"></button>`,
      )
      .join('')}</div>
    <label class="palette-free">Outra cor <input type="color" value="${currentColor().toLowerCase()}"></label>
    <button type="button" class="link reset">${icon('refresh', 14)} Restaurar padrão</button>`;
  document.body.append(pop);
  place(anchor);

  pop.addEventListener('click', (e) => {
    const sw = e.target.closest('.swatch');
    if (sw) return set(withColor(saved(), theme, cls, sw.dataset.color));
    if (e.target.closest('.reset')) return set(withoutColor(saved(), theme, cls));
    if (e.target.closest('.close')) return close();
  });
  pop.querySelector('input[type=color]').addEventListener('input', (e) => set(withColor(saved(), theme, cls, e.target.value)));

  // Opened by this same click: listen from the next event on.
  setTimeout(() => {
    document.addEventListener('pointerdown', onOutside, true);
    document.addEventListener('keydown', onKey, true);
  });
}
