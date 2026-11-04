// "Frases" tab: search, example sentences, selection and export.

import { $, invoke, dialog, toast, busy, escapeHtml, play } from './api.js';
import { getLoaded, openSection } from './config.js';
import { ankiMessage } from './anki.js';
import { renderDecks } from './estudar.js';

/** Selected cards, keyed by sentence so the same one isn't added twice. */
const selected = new Map();
let shown = [];
let currentWord = '';

function renderSelected() {
  $('#selCount').textContent = selected.size;
  $('#selected').innerHTML =
    [...selected.values()]
      .map((c) => `<li><span>${escapeHtml(c.sentence)}</span><button data-s="${escapeHtml(c.sentence)}" title="Remover">✕</button></li>`)
      .join('') || '<li class="empty">Marque frases para exportar.</li>';
  document.querySelectorAll('.ex').forEach((el) => {
    const on = selected.has(shown[el.dataset.i].japanese);
    el.classList.toggle('on', on);
    el.querySelector('.pick').checked = on;
  });
}

function sourceLabel(src) {
  return src.kind === 'tatoeba' ? `Tatoeba #${src.id}` : src.model;
}

function renderExamples(word) {
  const box = $('#examples');
  if (!shown.length) {
    box.innerHTML = '<div class="empty">Nenhuma frase encontrada. Importe o Tatoeba em Configurações ou use “Gerar com IA”.</div>';
    return;
  }
  box.innerHTML = shown
    .map(
      (ex, i) => `
    <div class="ex" data-i="${i}">
      <input type="checkbox" class="pick">
      <div class="sentence">${ex.html}</div>
      <div class="actions">
        <span class="src">${sourceLabel(ex.source)}</span>
        <button class="icon play" title="Ouvir">▶</button>
      </div>
      <input class="translation" value="${escapeHtml(ex.translation)}" placeholder="(sem tradução — digite uma)">
      <div class="meta">${ex.note ? escapeHtml(ex.note) : ''}${
        ex.contains_word ? '' : ` <span class="warn">⚠ a análise não encontrou 「${escapeHtml(word)}」 nesta frase</span>`
      }</div>
    </div>`,
    )
    .join('');
  renderSelected();
}

async function showWordInfo(word) {
  const box = $('#wordInfo');
  try {
    const info = await invoke('word_info', { word });
    const senses = (info.entries[0]?.senses ?? []).slice(0, 4);
    box.innerHTML = `<div class="head">${escapeHtml(word)}<small>${escapeHtml(info.reading)}</small></div>${
      senses.length
        ? '<ol>' + senses.map((s) => `<li>${escapeHtml(s.glosses.join('; '))} <span class="pos">${escapeHtml(s.pos.join(', '))}</span></li>`).join('') + '</ol>'
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

export function init() {
  $('#selected').addEventListener('click', (e) => {
    if (e.target.dataset.s) {
      selected.delete(e.target.dataset.s);
      renderSelected();
    }
  });
  $('#clearSel').addEventListener('click', () => {
    selected.clear();
    renderSelected();
  });

  $('#examples').addEventListener('click', (e) => {
    const el = e.target.closest('.ex');
    if (!el) return;
    const ex = shown[el.dataset.i];
    if (e.target.classList.contains('play')) return play(ex.japanese, e.target);
    if (e.target.classList.contains('translation')) return;
    if (selected.has(ex.japanese)) selected.delete(ex.japanese);
    else
      selected.set(ex.japanese, {
        word: currentWord,
        sentence: ex.japanese,
        translation: el.querySelector('.translation').value,
        note: ex.note ?? '',
      });
    renderSelected();
  });

  // Keep edited translations in sync with the selection.
  $('#examples').addEventListener('input', (e) => {
    if (!e.target.classList.contains('translation')) return;
    const ex = shown[e.target.closest('.ex').dataset.i];
    ex.translation = e.target.value;
    const card = selected.get(ex.japanese);
    if (card) card.translation = e.target.value;
  });

  $('#search').addEventListener('submit', (e) => {
    e.preventDefault();
    const word = $('#word').value.trim();
    const count = Number($('#count').value) || 5;
    currentWord = word;
    busy(e.submitter ?? $('#search button'), async () => {
      showWordInfo(word);
      shown = (await invoke('corpus_examples', { word, count })).map(flatten);
      if ($('#autoLlm').checked && shown.length < count) {
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
    busy(e.target, async () => {
      showWordInfo(word);
      const count = Number($('#count').value) || 5;
      shown = (await invoke('generated_examples', { word, count })).map(flatten);
      renderExamples(word);
    });
  });

  $('#toAnki').addEventListener('click', (e) => {
    const c = cards();
    if (!c) return;
    busy(e.target, async () => {
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
    busy(e.target, async () => {
      const path = await dialog.save({ defaultPath: 'japones.apkg', filters: [{ name: 'Anki', extensions: ['apkg'] }] });
      if (!path) return;
      await invoke('export_apkg', { path, cards: c, withAudio: $('#withAudio').checked });
      toast('Arquivo .apkg salvo. Importe no Anki em Arquivo › Importar.');
    });
  });

  $('#toPdf').addEventListener('click', (e) => {
    const c = cards();
    if (!c) return;
    busy(e.target, async () => {
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
    busy(e.target, async () => {
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
