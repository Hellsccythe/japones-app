// "Estudar" tab: decks, deck options, backup and the review session.

import { $, invoke, dialog, toast, busy, escapeHtml, play, stopAudio } from './api.js';
import { fillDeckName } from './frases.js';
import { tokensHtml } from './render.js';
import { icon } from './icons.js';

let decks = [];
let editing = null;
let studyDeck = null;
let studyCard = null;
let shownAt = 0;

/** Seconds → "<1min", "10min", "3h", "4d", "2 meses", "1,5 ano". */
function fmtInterval(secs) {
  if (secs < 60) return '<1min';
  if (secs < 3600) return Math.round(secs / 60) + 'min';
  if (secs < 86400) return Math.round(secs / 3600) + 'h';
  const days = secs / 86400;
  if (days < 30) return Math.round(days) + 'd';
  if (days < 365) {
    const m = Math.round(days / 30);
    return m + (m === 1 ? ' mês' : ' meses');
  }
  const y = Math.round((days / 365) * 10) / 10;
  return y.toLocaleString('pt-BR') + (y < 2 ? ' ano' : ' anos');
}

export async function renderDecks() {
  try {
    decks = await invoke('decks');
  } catch (e) {
    return toast(String(e), true);
  }
  const num = (n, cls) => `<td class="n ${n ? cls : 'zero'}">${n}</td>`;
  $('#deckList').innerHTML =
    decks
      .map(
        (d) => `<tr data-id="${d.id}">
      <td><button class="deck-name" title="Estudar">${escapeHtml(d.name)}</button><span class="total">${d.counts.total} cards</span></td>
      ${num(d.counts.new, 'new')}${num(d.counts.learning, 'learn')}${num(d.counts.review, 'due')}
      <td><button class="btn-icon gear" title="Opções" aria-label="Opções do deck">${icon('sliders', 16)}</button></td>
    </tr>`,
      )
      .join('') || '<tr><td colspan="5" class="empty">Nenhum deck ainda. Adicione frases em “Frases” ou importe um .apkg.</td></tr>';
  $('#deckNames').innerHTML = decks.map((d) => `<option value="${escapeHtml(d.name)}">`).join('');
  fillDeckName();
}

function openDeckEdit(deck) {
  editing = deck;
  const f = $('#deckEdit');
  f.elements.name.value = deck.name;
  f.elements.new_per_day.value = deck.new_per_day;
  f.elements.reviews_per_day.value = deck.reviews_per_day;
  f.classList.remove('hidden');
  f.elements.name.focus();
}

function startStudy(deck) {
  studyDeck = deck;
  $('#studyDeck').textContent = deck.name;
  $('#deckView').classList.add('hidden');
  $('#studyView').classList.remove('hidden');
  nextCard();
}

function leaveStudy() {
  studyDeck = null;
  studyCard = null;
  stopAudio();
  $('#studyView').classList.add('hidden');
  $('#deckView').classList.remove('hidden');
  renderDecks();
}

async function renderStudyCounts() {
  const d = (await invoke('decks')).find((x) => x.id === studyDeck.id);
  if (!d) return;
  const cur = { new: 'new', learning: 'learn', relearning: 'learn', review: 'due' }[studyCard?.state];
  $('#studyCounts').innerHTML = [
    ['new', d.counts.new],
    ['learn', d.counts.learning],
    ['due', d.counts.review],
  ]
    .map(([cls, n]) => `<span class="${cls}${cls === cur ? ' cur' : ''}">${n}</span>`)
    .join('');
}

async function nextCard() {
  try {
    studyCard = await invoke('next_card', { deckId: studyDeck.id });
  } catch (e) {
    return toast(String(e), true);
  }
  const done = !studyCard;
  $('#card').classList.toggle('hidden', done);
  $('#showRow').classList.toggle('hidden', done);
  $('#rateRow').classList.add('hidden');
  $('#studyDone').classList.toggle('hidden', !done);
  renderStudyCounts().catch(() => {});
  if (done) return;
  const colored = studyCard.tokens?.length;
  if (colored) $('#cardSentence').innerHTML = tokensHtml(studyCard.tokens, { ruby: false });
  else $('#cardSentence').textContent = studyCard.sentence;
  $('#cardWord').textContent = studyCard.word;
  $('#cardFurigana').innerHTML = colored ? tokensHtml(studyCard.tokens) : studyCard.furigana_html;
  $('#cardTranslation').textContent = studyCard.translation;
  $('#cardNote').textContent = studyCard.note;
  $('#cardSentence').classList.remove('hidden');
  $('#cardBack').classList.add('hidden');
  document.querySelectorAll('.rate').forEach((b) => {
    b.querySelector('small').textContent = fmtInterval(studyCard.intervals[b.dataset.r - 1]);
  });
  shownAt = performance.now();
}

function showAnswer() {
  if (!studyCard) return;
  // The furigana version replaces the plain sentence instead of repeating it.
  $('#cardSentence').classList.add('hidden');
  $('#cardBack').classList.remove('hidden');
  $('#showRow').classList.add('hidden');
  $('#rateRow').classList.remove('hidden');
  if ($('#autoPlay').checked) $('#cardPlay').click();
}

async function rate(r) {
  if (!studyCard || $('#rateRow').classList.contains('hidden')) return;
  // Cap it so a card left on screen during a break doesn't skew the log.
  const tookMs = Math.min(Math.round(performance.now() - shownAt), 60000);
  const cardId = studyCard.card_id;
  studyCard = null;
  try {
    await invoke('answer_card', { cardId, rating: r, tookMs });
  } catch (e) {
    toast(String(e), true);
  }
  nextCard();
}

export function init() {
  $('#deckList').addEventListener('click', (e) => {
    const row = e.target.closest('tr[data-id]');
    if (!row) return;
    const deck = decks.find((d) => d.id === Number(row.dataset.id));
    if (e.target.closest('.deck-name')) startStudy(deck);
    else if (e.target.closest('.gear')) openDeckEdit(deck);
  });

  $('#closeDeckEdit').addEventListener('click', () => $('#deckEdit').classList.add('hidden'));

  $('#deckEdit').addEventListener('submit', (e) => {
    e.preventDefault();
    const f = e.target.elements;
    busy(e.submitter, async () => {
      await invoke('update_deck', {
        deck: {
          id: editing.id,
          name: f.name.value.trim(),
          new_per_day: Number(f.new_per_day.value) || 0,
          reviews_per_day: Number(f.reviews_per_day.value) || 0,
        },
      });
      $('#deckEdit').classList.add('hidden');
      toast('Deck salvo.');
      renderDecks();
    });
  });

  $('#deleteDeck').addEventListener('click', async () => {
    const ok = await dialog.ask(
      `Apagar o deck “${editing.name}” com ${editing.counts.total} cards e todo o histórico? Não dá para desfazer (a não ser por um backup).`,
      { kind: 'warning', okLabel: 'Sim', cancelLabel: 'Não' },
    );
    if (!ok) return;
    await invoke('delete_deck', { id: editing.id }).catch((err) => toast(String(err), true));
    $('#deckEdit').classList.add('hidden');
    renderDecks();
  });

  $('#exportDeck').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const name = editing.name.replace(/[\\/:*?"<>|]+/g, '_') + '.apkg';
      const path = await dialog.save({ defaultPath: name, filters: [{ name: 'Anki', extensions: ['apkg'] }] });
      if (!path) return;
      const n = await invoke('export_deck_apkg', { deckId: editing.id, path, withAudio: $('#deckAudio').checked });
      toast(`${n} cards exportados, com o progresso e o histórico de revisões. Importe no Anki em Arquivo › Importar.`);
    }),
  );

  $('#newDeck').addEventListener('click', async (e) => {
    const name = prompt('Nome do deck (use :: para subdecks, ex.: Japonês::Verbos)');
    if (!name?.trim()) return;
    await busy(e.currentTarget, async () => {
      await invoke('create_deck', { name: name.trim() });
      renderDecks();
    });
  });

  $('#importApkg').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const path = await dialog.open({ multiple: false, filters: [{ name: 'Anki', extensions: ['apkg', 'colpkg'] }] });
      if (!path) return;
      const base = String(path).split(/[\\/]/).pop().replace(/\.\w+$/, '');
      const deck = prompt('Importar para qual deck?', base)?.trim();
      if (!deck) return;
      const r = await invoke('import_apkg', { path, deck });
      toast(`${r.found} notas no arquivo, ${r.added} novas em ${deck}.`);
      renderDecks();
    }),
  );

  $('#backupStudy').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const day = new Date().toISOString().slice(0, 10);
      const path = await dialog.save({ defaultPath: `japones-backup-${day}.db`, filters: [{ name: 'Backup', extensions: ['db'] }] });
      if (!path) return;
      await invoke('backup_study', { path });
      toast('Backup salvo.');
    }),
  );

  $('#restoreStudy').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const path = await dialog.open({ multiple: false, filters: [{ name: 'Backup', extensions: ['db'] }] });
      if (!path) return;
      const ok = await dialog.ask('Substituir todos os decks e o histórico deste aparelho pelo backup?', { kind: 'warning', okLabel: 'Sim', cancelLabel: 'Não' });
      if (!ok) return;
      await invoke('restore_study', { path });
      toast('Backup restaurado.');
      renderDecks();
    }),
  );

  $('#leaveStudy').addEventListener('click', leaveStudy);
  $('#showAnswer').addEventListener('click', showAnswer);
  $('#cardPlay').addEventListener('click', (e) => studyCard && play(studyCard.sentence, e.currentTarget));
  $('#autoPlay').addEventListener('change', (e) => {
    try {
      localStorage.setItem('autoPlay', e.target.checked ? '1' : '0');
    } catch {}
  });
  try {
    $('#autoPlay').checked = localStorage.getItem('autoPlay') !== '0';
  } catch {}

  $('#rateRow').addEventListener('click', (e) => {
    const b = e.target.closest('.rate');
    if (b) rate(Number(b.dataset.r));
  });

  document.addEventListener('keydown', (e) => {
    if (!studyDeck || e.target.closest?.('input, textarea, select') || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      if ($('#rateRow').classList.contains('hidden')) showAnswer();
      else rate(3);
    } else if (['1', '2', '3', '4'].includes(e.key)) rate(Number(e.key));
    else if (e.key === 'r' || e.key === 'R') $('#cardPlay').click();
    else if (e.key === 'Escape') leaveStudy();
  });
}
