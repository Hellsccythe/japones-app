// "Frases" tab: search, sentence cards with the vocabulary breakdown, selection bar and export.

import { $, invoke, dialog, toast, busy, escapeHtml, play } from './api.js';
import { getLoaded, openSection } from './config.js';
import { ankiMessage } from './anki.js';
import { renderDecks } from './estudar.js';
import { icon } from './icons.js';
import { tokensHtml, vocabHtml, legendHtml, copyText } from './render.js';

/** Selected cards, keyed by sentence so the same one isn't added twice. */
const selected = new Map();
/** Sentences on screen; each may carry `vocab` once its details were opened. */
let shown = [];
let currentWord = '';

const exampleAt = (el) => shown[el.closest('.ex').dataset.i];

function renderSelected() {
  const n = selected.size;
  $('#selCount').textContent = n;
  $('#selLabel').textContent = n === 1 ? 'selecionada' : 'selecionadas';
  $('#selBar').classList.toggle('hidden', !n);
  document.body.classList.toggle('has-sel', n > 0);
  if (!n) closeMenu();
  document.querySelectorAll('.ex').forEach((el) => {
    const on = selected.has(shown[el.dataset.i].japanese);
    el.classList.toggle('on', on);
    const pick = el.querySelector('.pick');
    pick.classList.toggle('on', on);
    pick.setAttribute('aria-pressed', String(on));
    pick.innerHTML = icon(on ? 'check' : 'plus');
    pick.title = on ? 'Tirar da seleção' : 'Selecionar';
    pick.setAttribute('aria-label', pick.title);
  });
}

function sourceLabel(src) {
  return src.kind === 'tatoeba' ? `Tatoeba #${src.id}` : src.model;
}

function sentenceHtml(ex) {
  return ex.tokens?.length ? tokensHtml(ex.tokens) : ex.html;
}

function renderExamples(word) {
  const box = $('#examples');
  if (!shown.length) {
    box.innerHTML = '<div class="empty">Nenhuma frase encontrada. Baixe as frases do Tatoeba em Configurações ou use “Gerar com IA”.</div>';
    return;
  }
  box.innerHTML = shown
    .map(
      (ex, i) => `
    <article class="ex" data-i="${i}">
      <div class="sentence jp">${sentenceHtml(ex)}</div>
      <input class="translation" value="${escapeHtml(ex.translation)}" placeholder="(sem tradução — digite uma)" aria-label="Tradução">
      ${
        ex.note || !ex.contains_word
          ? `<div class="meta">${ex.note ? escapeHtml(ex.note) : ''}${
              ex.contains_word ? '' : ` <span class="warn">${icon('alert', 14)} a análise não encontrou 「${escapeHtml(word)}」 nesta frase</span>`
            }</div>`
          : ''
      }
      <div class="ex-actions">
        <button type="button" class="btn-icon play" title="Ouvir" aria-label="Ouvir">${icon('volume')}</button>
        <button type="button" class="pill details" aria-expanded="false">${icon('chevron-down')}<span>Detalhes</span></button>
        <button type="button" class="btn-icon pick" title="Selecionar" aria-label="Selecionar" aria-pressed="false">${icon('plus')}</button>
        <span class="grow"></span>
        <span class="tag muted">${escapeHtml(sourceLabel(ex.source))}</span>
      </div>
      <div class="vocab hidden"></div>
    </article>`,
    )
    .join('');
  renderSelected();
}

/** Vocabulary box of one card: header with actions, word grid and the color legend. */
function renderVocab(el, ex) {
  el.querySelector('.vocab').innerHTML = `
    <div class="vocab-head">
      <strong>Vocabulário</strong>
      <span class="grow"></span>
      <button type="button" class="pill copy" title="Copiar a frase e o vocabulário">${icon('copy', 16)}<span>Copiar</span></button>
    </div>
    <div class="v-grid">${vocabHtml(ex.vocab)}</div>
    <div class="legend">${legendHtml()}</div>`;
}

async function toggleDetails(btn) {
  const el = btn.closest('.ex');
  const ex = exampleAt(el);
  const box = el.querySelector('.vocab');
  const open = box.classList.contains('hidden');
  if (open && !ex.vocab) {
    await busy(btn, async () => {
      ex.vocab = await invoke('vocabulary', { text: ex.japanese });
      renderVocab(el, ex);
    });
    if (!ex.vocab) return;
  }
  box.classList.toggle('hidden', !open);
  btn.classList.toggle('on', open);
  btn.setAttribute('aria-expanded', String(open));
  btn.querySelector('.ic').outerHTML = icon(open ? 'chevron-up' : 'chevron-down');
}

function copyVocab(btn) {
  const ex = exampleAt(btn);
  navigator.clipboard
    .writeText(copyText(ex.japanese, ex.translation, ex.vocab))
    .then(() => toast('Copiado.'))
    .catch((e) => toast(String(e), true));
}

function togglePick(btn) {
  const el = btn.closest('.ex');
  const ex = exampleAt(el);
  if (selected.has(ex.japanese)) selected.delete(ex.japanese);
  else
    selected.set(ex.japanese, {
      word: currentWord,
      sentence: ex.japanese,
      translation: el.querySelector('.translation').value,
      note: ex.note ?? '',
    });
  renderSelected();
}

/** Dictionary line: word, reading, part-of-speech tag and first meaning; `+N` shows the other senses. */
async function showWordInfo(word) {
  const box = $('#wordInfo');
  try {
    const info = await invoke('word_info', { word });
    const senses = info.entries[0]?.senses ?? [];
    if (!senses.length) {
      box.classList.add('hidden');
      return;
    }
    const [first, ...rest] = senses;
    const pos = first.pos[0] ? `<span class="tag muted">${escapeHtml(first.pos[0])}</span>` : '';
    box.innerHTML = `<div class="wi-line">
        <span class="wi-word jp">${escapeHtml(word)}</span>
        <span class="wi-reading jp">${escapeHtml(info.reading)}</span>
        ${pos}
        <span class="wi-gloss">${escapeHtml(first.glosses.join('; '))}</span>
        ${rest.length ? `<button type="button" class="pill more" aria-expanded="false">+${rest.length}</button>` : ''}
      </div>
      ${
        rest.length
          ? `<ol class="wi-more hidden" start="2">${rest
              .map((s) => `<li>${escapeHtml(s.glosses.join('; '))} <span class="pos">${escapeHtml(s.pos.join(', '))}</span></li>`)
              .join('')}</ol>`
          : ''
      }`;
    box.classList.remove('hidden');
  } catch {
    box.classList.add('hidden');
  }
}

/** ExampleView arrives with the example fields flattened in already; normalise. */
function flatten(v) {
  return { ...v, translation: v.translation ?? '' };
}

function cards() {
  if (!selected.size) {
    toast('Selecione ao menos uma frase.', true);
    return null;
  }
  return [...selected.values()];
}

/** The deck field's starting value: the last deck used, else the one in the settings. */
export function fillDeckName() {
  if ($('#deckName').value) return;
  try {
    $('#deckName').value = localStorage.getItem('lastDeck') ?? '';
  } catch {}
  if (!$('#deckName').value) $('#deckName').value = getLoaded().deck || 'Japonês';
}

// ---------- export menu ----------
function closeMenu() {
  $('#exportMenu').classList.add('hidden');
  $('#exportBtn').setAttribute('aria-expanded', 'false');
}

function toggleMenu() {
  const open = $('#exportMenu').classList.contains('hidden');
  $('#exportMenu').classList.toggle('hidden', !open);
  $('#exportBtn').setAttribute('aria-expanded', String(open));
}

/** How many sentences a search asks for, and whether the AI tops it up (Configurações). */
const searchCount = () => Number(getLoaded().examples_count) || 5;
const fillWithLlm = () => getLoaded().fill_with_llm ?? true;

export function init() {
  $('#examples').addEventListener('click', (e) => {
    const btn = e.target.closest('button');
    if (!btn || !btn.closest('.ex')) return;
    if (btn.classList.contains('play')) return play(exampleAt(btn).japanese, btn);
    if (btn.classList.contains('details')) return toggleDetails(btn);
    if (btn.classList.contains('pick')) return togglePick(btn);
    if (btn.classList.contains('copy')) return copyVocab(btn);
  });

  // Keep edited translations in sync with the selection.
  $('#examples').addEventListener('input', (e) => {
    if (!e.target.classList.contains('translation')) return;
    const ex = exampleAt(e.target);
    ex.translation = e.target.value;
    const card = selected.get(ex.japanese);
    if (card) card.translation = e.target.value;
  });

  $('#wordInfo').addEventListener('click', (e) => {
    const more = e.target.closest('.more');
    if (!more) return;
    const list = $('#wordInfo .wi-more');
    const open = list.classList.toggle('hidden') === false;
    more.setAttribute('aria-expanded', String(open));
    more.classList.toggle('on', open);
  });

  $('#search').addEventListener('submit', (e) => {
    e.preventDefault();
    const word = $('#word').value.trim();
    const count = searchCount();
    currentWord = word;
    busy(e.submitter ?? $('#search [type=submit]'), async () => {
      showWordInfo(word);
      shown = (await invoke('corpus_examples', { word, count })).map(flatten);
      if (fillWithLlm() && shown.length < count) {
        try {
          const more = await invoke('generated_examples', { word, count: count - shown.length });
          shown.push(...more.map(flatten));
        } catch (err) {
          toast('IA: ' + err, true);
        }
      }
      renderExamples(word);
    });
  });

  $('#generate').addEventListener('click', (e) => {
    const word = $('#word').value.trim();
    if (!word) return $('#word').focus();
    currentWord = word;
    busy(e.currentTarget, async () => {
      showWordInfo(word);
      shown = (await invoke('generated_examples', { word, count: searchCount() })).map(flatten);
      renderExamples(word);
    });
  });

  $('#exportBtn').addEventListener('click', toggleMenu);
  document.addEventListener('click', (e) => {
    if (!e.target.closest('.menu-wrap')) closeMenu();
  });
  document.addEventListener('keydown', (e) => e.key === 'Escape' && closeMenu());

  $('#clearSel').addEventListener('click', () => {
    selected.clear();
    renderSelected();
  });

  $('#toAnki').addEventListener('click', (e) => {
    const c = cards();
    if (!c) return;
    closeMenu();
    busy(e.currentTarget, async () => {
      try {
        const n = await invoke('export_anki_connect', { cards: c, withAudio: $('#withAudio').checked });
        toast(`${n} de ${c.length} cards adicionados ao Anki${n < c.length ? ' (os outros já existiam)' : ''}.`);
      } catch (err) {
        const m = ankiMessage(err);
        const actions = m.help
          ? [
              { label: 'Ver como instalar', run: () => openSection('anki') },
              { label: 'Salvar .apkg', run: () => $('#toApkg').click() },
            ]
          : [];
        toast(m.text, true, actions);
      }
    });
  });

  $('#toApkg').addEventListener('click', (e) => {
    const c = cards();
    if (!c) return;
    closeMenu();
    busy(e.currentTarget, async () => {
      const path = await dialog.save({ defaultPath: 'japones.apkg', filters: [{ name: 'Anki', extensions: ['apkg'] }] });
      if (!path) return;
      await invoke('export_apkg', { path, cards: c, withAudio: $('#withAudio').checked });
      toast('Arquivo .apkg salvo. Importe no Anki em Arquivo › Importar.');
    });
  });

  $('#toPdf').addEventListener('click', (e) => {
    const c = cards();
    if (!c) return;
    closeMenu();
    busy(e.currentTarget, async () => {
      const words = [...new Set(c.map((x) => x.word))];
      const path = await dialog.save({ defaultPath: words.join('_') + '.pdf', filters: [{ name: 'PDF', extensions: ['pdf'] }] });
      if (!path) return;
      await invoke('export_pdf', { path, title: words.join('・'), cards: c, furigana: $('#pdfFurigana').checked });
      toast('PDF salvo.');
    });
  });

  $('#toDeck').addEventListener('click', (e) => {
    const c = cards();
    if (!c) return;
    const deck = $('#deckName').value.trim();
    if (!deck) return $('#deckName').focus();
    busy(e.currentTarget, async () => {
      const n = await invoke('add_to_deck', { deck, cards: c });
      toast(`${n} de ${c.length} cards adicionados a ${deck}${n < c.length ? ' (os outros já estavam no deck)' : ''}.`);
      try {
        localStorage.setItem('lastDeck', deck);
      } catch {}
      renderDecks();
    });
  });

  renderSelected();
}
