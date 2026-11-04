// Pure HTML/text builders for sentences and their vocabulary (no Tauri here, so node can test them).

import { CLASSES, CLASS_NAMES } from './colors.js';

export function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
}

const cls = (c) => (CLASSES.includes(c) ? c : 'other');

/** Furigana segments as HTML with `<ruby>` on the ones that have a reading. */
export function segmentsHtml(segments) {
  return (segments ?? [])
    .map((s) => (s.reading ? `<ruby>${escapeHtml(s.text)}<rt>${escapeHtml(s.reading)}</rt></ruby>` : escapeHtml(s.text)))
    .join('');
}

/** A sentence as one colored `<span class="w w-<class>">` per token. */
export function tokensHtml(tokens) {
  return (tokens ?? []).map((t) => `<span class="w w-${cls(t.class)}">${segmentsHtml(t.segments)}</span>`).join('');
}

const noBrackets = (s) => String(s ?? '').replace(/\[/g, '(').replace(/\]/g, ')');

/** Plain text for the clipboard: sentence, translation, blank line, one `word (reading) = meaning` per line. */
export function copyText(sentence, translation, vocab) {
  const lines = (vocab ?? []).map(
    (e) => `${e.base}${e.reading ? ` (${e.reading})` : ''} = ${e.gloss ? e.gloss : '—'}`,
  );
  return noBrackets(`${sentence}\n${translation}\n\n${lines.join('\n')}`);
}

/** Vocabulary grid rows: dictionary form in its class color, reading, meaning (or —). */
export function vocabHtml(vocab) {
  if (!vocab?.length) return '<div class="empty">Nenhuma palavra para mostrar.</div>';
  return vocab
    .map(
      (e) => `<div class="v-row">
        <span class="v-base jp w w-${cls(e.class)}">${escapeHtml(e.base)}</span>
        <span class="v-reading jp">${e.reading ? escapeHtml(e.reading) : ''}</span>
        <span class="v-gloss${e.gloss ? '' : ' none'}">${e.gloss ? escapeHtml(e.gloss) : '—'}</span>
      </div>`,
    )
    .join('');
}

/** Clickable color dots, one per word class. */
export function legendHtml() {
  return CLASSES.map(
    (c) =>
      `<button type="button" class="legend-item" data-class="${c}" title="Trocar a cor" aria-label="Trocar a cor de ${CLASS_NAMES[c]}"><i class="dot-${c}"></i>${CLASS_NAMES[c]}</button>`,
  ).join('');
}
