// "Texto" tab: furigana, audio, breakdown and PDF for any text.

import { $, invoke, dialog, toast, busy, play, escapeHtml } from './api.js';
import { ensureReady, openQuickSetup } from './setup.js';
import { tokensHtml, vocabHtml, legendHtml, copyText } from './render.js';
import { icon } from './icons.js';
import { colorStore } from './config.js';
import { openPalette } from './palette.js';
import { needsAiSetup } from './ia.js';

/** Lines on screen; each gets `vocab` and `translation` once opened / translated. */
let lines = [];

/** Breakdown of one line: translation, vocabulary, actions and the color legend. */
function detailsHtml(line) {
  return `
    <div class="t-tr${line.translation ? '' : ' none'}">${
      line.translation ? escapeHtml(line.translation) : 'Sem tradução: use “Traduzir com IA”.'
    }</div>
    <div class="vocab-head">
      <strong>Vocabulário</strong>
      <span class="grow"></span>
      <button type="button" class="pill translate" title="Traduzir a frase e os significados">${icon('sparkles', 16)}<span>Traduzir com IA</span></button>
      <button type="button" class="pill copy" title="Copiar a frase, a tradução e o vocabulário">${icon('copy', 16)}<span>Copiar</span></button>
    </div>
    <div class="v-grid">${vocabHtml(line.vocab)}</div>
    <div class="legend">${legendHtml()}</div>`;
}

const lineOf = (el) => lines[el.closest('.t-line').dataset.i];

async function toggleDetails(btn) {
  const row = btn.closest('.t-line');
  const line = lineOf(btn);
  const box = row.querySelector('.vocab');
  const open = box.classList.contains('hidden');
  if (open && !line.vocab) {
    await busy(btn, async () => {
      line.vocab = await invoke('vocabulary', { text: line.text });
      box.innerHTML = detailsHtml(line);
    });
    if (!line.vocab) return;
  }
  box.classList.toggle('hidden', !open);
  btn.classList.toggle('on', open);
  btn.setAttribute('aria-expanded', String(open));
  btn.querySelector('.ic').outerHTML = icon(open ? 'chevron-up' : 'chevron-down');
}

/** Translates the sentence and its vocabulary (the local model runs them in turn). */
function translateLine(btn) {
  const row = btn.closest('.t-line');
  const line = lineOf(btn);
  return busy(btn, async () => {
    try {
      const [translation, vocab] = await Promise.all([
        invoke('translate_sentence', { text: line.text }),
        invoke('translate_vocabulary', { entries: line.vocab }),
      ]);
      Object.assign(line, { translation, vocab });
      row.querySelector('.vocab').innerHTML = detailsHtml(line);
    } catch (err) {
      if (needsAiSetup(err)) openQuickSetup('ia');
      else toast('IA: ' + err, true);
    }
  });
}

function copyLine(btn) {
  const line = lineOf(btn);
  navigator.clipboard
    .writeText(copyText(line.text, line.translation ?? '', line.vocab))
    .then(() => toast('Copiado.'))
    .catch((e) => toast(String(e), true));
}

export function init() {
  $('#showFurigana').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const texts = $('#text').value.split('\n').map((l) => l.trim()).filter(Boolean);
      const out = await Promise.all(texts.map((text) => invoke('furigana', { text })));
      lines = texts.map((text) => ({ text }));
      $('#textOut').innerHTML = out
        .map(
          (f, i) => `<div class="t-line" data-i="${i}">
            <p><button type="button" class="btn-icon play" title="Ouvir" aria-label="Ouvir">${icon('volume')}</button><span>${
              f.tokens?.length ? tokensHtml(f.tokens) : f.html
            }</span>
            <button type="button" class="pill details" aria-expanded="false">${icon('chevron-down')}<span>Detalhes</span></button></p>
            <div class="vocab hidden"></div>
          </div>`,
        )
        .join('');
    }),
  );

  $('#textOut').addEventListener('click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    if (b.classList.contains('play')) {
      const text = lineOf(b).text;
      return ensureReady('voz').then((ok) => ok && play(text, b));
    }
    if (b.classList.contains('details')) return toggleDetails(b);
    if (b.classList.contains('translate')) return translateLine(b);
    if (b.classList.contains('copy')) return copyLine(b);
    if (b.dataset.class) return openPalette(b, b.dataset.class, colorStore);
  });

  $('#playText').addEventListener('click', (e) => {
    const text = $('#text').value.trim();
    const btn = e.currentTarget;
    if (text) ensureReady('voz').then((ok) => ok && play(text, btn));
  });

  $('#hideRt').addEventListener('change', (e) => $('#textOut').classList.toggle('hide-rt', e.target.checked));

  $('#textPdf').addEventListener('click', (e) => {
    const text = $('#text').value.trim();
    if (!text) return toast('Digite um texto.', true);
    busy(e.currentTarget, async () => {
      const path = await dialog.save({ defaultPath: 'texto.pdf', filters: [{ name: 'PDF', extensions: ['pdf'] }] });
      if (!path) return;
      await invoke('export_text_pdf', { path, title: 'Texto', text, furigana: $('#textPdfFurigana').checked });
      toast('PDF salvo.');
    });
  });
}
