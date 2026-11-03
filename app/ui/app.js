const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;

const $ = (s) => document.querySelector(s);

// AnkiConnect and the VOICEVOX program only exist on the PC.
const isAndroid = /Android/i.test(navigator.userAgent);
document.documentElement.classList.toggle('android', isAndroid);

// ---------- helpers ----------
let toastTimer;
function toast(msg, isError = false) {
  const t = $('#toast');
  t.textContent = msg;
  t.className = 'toast' + (isError ? ' error' : '');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add('hidden'), isError ? 7000 : 3500);
}

/** Runs `fn` with the button disabled; shows errors as a toast. */
async function busy(btn, fn) {
  const label = btn.textContent;
  btn.disabled = true;
  btn.textContent = '…';
  try {
    return await fn();
  } catch (e) {
    toast(String(e), true);
  } finally {
    btn.disabled = false;
    btn.textContent = label;
  }
}

function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
}

let currentAudio;
async function play(text, btn) {
  await busy(btn, async () => {
    const b64 = await invoke('speak', { text });
    currentAudio?.pause();
    currentAudio = new Audio('data:audio/wav;base64,' + b64);
    await currentAudio.play();
  });
}

// ---------- tabs & status ----------
document.querySelectorAll('.tab').forEach((b) =>
  b.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach((x) => x.classList.toggle('active', x === b));
    document.querySelectorAll('.panel').forEach((p) => p.classList.toggle('active', p.id === 'tab-' + b.dataset.tab));
  }),
);

async function refreshStatus() {
  try {
    const s = await invoke('status');
    const parts = [
      s.tatoeba_indexed ? `${s.tatoeba_indexed.toLocaleString()} frases do Tatoeba` : 'Tatoeba não importado',
      s.dictionary_loaded ? 'dicionário ok' : 'sem dicionário',
      s.llm_ready ? `IA: ${s.llm_name}` : `IA: ${s.llm_name} (não configurada)`,
      s.voice_ready ? `voz: ${s.voice_name}` : 'voz não instalada',
    ];
    $('#status').textContent = parts.join(' · ');
  } catch (e) {
    $('#status').textContent = String(e);
  }
}

// ---------- examples ----------
/** Selected cards, keyed by sentence so the same one isn't added twice. */
const selected = new Map();
let shown = [];

function renderSelected() {
  $('#selCount').textContent = selected.size;
  $('#selected').innerHTML = [...selected.values()]
    .map((c) => `<li><span>${escapeHtml(c.sentence)}</span><button data-s="${escapeHtml(c.sentence)}" title="Remover">✕</button></li>`)
    .join('') || '<li class="empty">Marque frases para exportar.</li>';
  document.querySelectorAll('.ex').forEach((el) => {
    const on = selected.has(shown[el.dataset.i].japanese);
    el.classList.toggle('on', on);
    el.querySelector('.pick').checked = on;
  });
}

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

let currentWord = '';

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

/** ExampleView arrives with the example fields flattened in already; normalise. */
function flatten(v) {
  return { ...v, translation: v.translation ?? '' };
}

// ---------- export ----------
function cards() {
  if (!selected.size) {
    toast('Selecione ao menos uma frase.', true);
    return null;
  }
  return [...selected.values()];
}

$('#toAnki').addEventListener('click', (e) => {
  const c = cards();
  if (!c) return;
  busy(e.target, async () => {
    const n = await invoke('export_anki_connect', { cards: c, withAudio: $('#withAudio').checked });
    toast(`${n} de ${c.length} cards adicionados ao Anki${n < c.length ? ' (os outros já existiam)' : ''}.`);
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

// ---------- estudar ----------
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

async function renderDecks() {
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
      <td><button class="gear" title="Opções">⚙</button></td>
    </tr>`,
      )
      .join('') || '<tr><td colspan="5" class="empty">Nenhum deck ainda. Adicione frases em “Frases” ou importe um .apkg.</td></tr>';
  $('#deckNames').innerHTML = decks.map((d) => `<option value="${escapeHtml(d.name)}">`).join('');
  if (!$('#deckName').value) {
    try {
      $('#deckName').value = localStorage.getItem('lastDeck') ?? '';
    } catch {}
    if (!$('#deckName').value) $('#deckName').value = loaded?.deck || 'Japonês';
  }
}

$('#deckList').addEventListener('click', (e) => {
  const row = e.target.closest('tr[data-id]');
  if (!row) return;
  const deck = decks.find((d) => d.id === Number(row.dataset.id));
  if (e.target.classList.contains('deck-name')) startStudy(deck);
  else if (e.target.classList.contains('gear')) openDeckEdit(deck);
});

function openDeckEdit(deck) {
  editing = deck;
  const f = $('#deckEdit');
  f.elements.name.value = deck.name;
  f.elements.new_per_day.value = deck.new_per_day;
  f.elements.reviews_per_day.value = deck.reviews_per_day;
  f.classList.remove('hidden');
  f.elements.name.focus();
}

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
  busy(e.target, async () => {
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
  await busy(e.target, async () => {
    await invoke('create_deck', { name: name.trim() });
    renderDecks();
  });
});

$('#importApkg').addEventListener('click', (e) =>
  busy(e.target, async () => {
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
  busy(e.target, async () => {
    const day = new Date().toISOString().slice(0, 10);
    const path = await dialog.save({ defaultPath: `japones-backup-${day}.db`, filters: [{ name: 'Backup', extensions: ['db'] }] });
    if (!path) return;
    await invoke('backup_study', { path });
    toast('Backup salvo.');
  }),
);

$('#restoreStudy').addEventListener('click', (e) =>
  busy(e.target, async () => {
    const path = await dialog.open({ multiple: false, filters: [{ name: 'Backup', extensions: ['db'] }] });
    if (!path) return;
    const ok = await dialog.ask('Substituir todos os decks e o histórico deste aparelho pelo backup?', { kind: 'warning', okLabel: 'Sim', cancelLabel: 'Não' });
    if (!ok) return;
    await invoke('restore_study', { path });
    toast('Backup restaurado.');
    renderDecks();
  }),
);

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
  currentAudio?.pause();
  $('#studyView').classList.add('hidden');
  $('#deckView').classList.remove('hidden');
  renderDecks();
}
$('#leaveStudy').addEventListener('click', leaveStudy);

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
  $('#cardSentence').textContent = studyCard.sentence;
  $('#cardWord').textContent = studyCard.word;
  $('#cardFurigana').innerHTML = studyCard.furigana_html;
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

// ---------- texto ----------
$('#showFurigana').addEventListener('click', (e) =>
  busy(e.target, async () => {
    const lines = $('#text').value.split('\n').filter((l) => l.trim());
    const out = await Promise.all(lines.map((l) => invoke('furigana', { text: l.trim() })));
    $('#textOut').innerHTML = out
      .map((f, i) => `<p><button class="icon play" data-i="${i}" title="Ouvir">▶</button><span>${f.html}</span></p>`)
      .join('');
    $('#textOut').dataset.lines = JSON.stringify(lines);
  }),
);

$('#textOut').addEventListener('click', (e) => {
  if (!e.target.classList.contains('play')) return;
  const lines = JSON.parse($('#textOut').dataset.lines || '[]');
  play(lines[e.target.dataset.i], e.target);
});

$('#playText').addEventListener('click', (e) => {
  const text = $('#text').value.trim();
  if (text) play(text, e.target);
});

$('#hideRt').addEventListener('change', (e) => $('#textOut').classList.toggle('hide-rt', e.target.checked));

$('#textPdf').addEventListener('click', (e) => {
  const text = $('#text').value.trim();
  if (!text) return toast('Digite um texto.', true);
  busy(e.target, async () => {
    const path = await dialog.save({ defaultPath: 'texto.pdf', filters: [{ name: 'PDF', extensions: ['pdf'] }] });
    if (!path) return;
    await invoke('export_text_pdf', { path, title: 'Texto', text, furigana: $('#textPdfFurigana').checked });
    toast('PDF salvo.');
  });
});

// ---------- configurações ----------
const form = $('#settings');
/** Last settings from the backend; saving starts from these so fields without inputs keep their values. */
let loaded = {};

async function loadSettings() {
  const s = await invoke('get_settings');
  loaded = s;
  await Promise.all([renderModels(), renderVoices()]);
  for (const [k, v] of Object.entries(s)) {
    const el = form.elements[k];
    if (!el) continue;
    if (k === 'speaker' && !el.options.length) el.add(new Option(`Voz ${v}`, v));
    if (el.type === 'checkbox') el.checked = v;
    else el.value = Array.isArray(v) ? v.join(', ') : v;
  }
  showProvider();
  showBackend();
  loadSpeakers().catch(() => {});
}

// ---------- IA ----------
function showProvider() {
  const local = form.elements.llm_provider.value === 'local';
  $('#localBox').classList.toggle('hidden', !local);
  $('#claudeBox').classList.toggle('hidden', local);
}
$('#llmProvider').addEventListener('change', showProvider);

const gb = (bytes) => (bytes / 1e9).toLocaleString('pt-BR', { maximumFractionDigits: 1, minimumFractionDigits: 1 }) + ' GB';
/** Model ids with a download running in this window. */
const downloading = new Set();

async function renderModels() {
  const models = await invoke('models');
  const current = form.elements.local_model?.value || loaded.local_model;
  $('#models').innerHTML = models
    .map((m) => {
      const state = m.downloaded
        ? 'baixado'
        : m.partial_bytes
          ? `${Math.floor((m.partial_bytes * 100) / m.size)}% baixado`
          : 'não baixado';
      const action = m.downloaded
        ? `<button type="button" class="secondary" data-delete="${m.id}">Apagar</button>`
        : `<button type="button" data-download="${m.id}"${downloading.has(m.id) ? ' disabled' : ''}>${m.partial_bytes ? 'Continuar' : 'Baixar'}</button>`;
      const done = downloading.has(m.id) || m.partial_bytes ? `<progress id="prog-${m.id}" max="${m.size}" value="${m.partial_bytes}"></progress>` : '';
      return `<li class="${m.id === current ? 'active' : ''}">
        <input type="radio" name="local_model" value="${m.id}"${m.id === current ? ' checked' : ''}>
        <div><strong>${escapeHtml(m.name)}</strong>
          <div class="meta">${gb(m.size)} · ${escapeHtml(m.recommended_for)} · ${escapeHtml(m.license)} · <span id="state-${m.id}">${state}</span></div></div>
        <div class="actions">${action}</div>
        ${done}
      </li>`;
    })
    .join('');
}

$('#models').addEventListener('change', (e) => {
  if (e.target.name === 'local_model') {
    document.querySelectorAll('#models li').forEach((li) => li.classList.toggle('active', li.contains(e.target)));
  }
});

$('#models').addEventListener('click', async (e) => {
  const { download: dl, delete: del } = e.target.dataset;
  if (dl) {
    downloading.add(dl);
    await renderModels();
    try {
      await invoke('download_model', { id: dl });
      toast('Modelo baixado e verificado.');
    } catch (err) {
      toast(String(err), true);
    } finally {
      downloading.delete(dl);
      await renderModels();
      refreshStatus();
    }
  } else if (del && (await dialog.ask('Apagar este modelo do aparelho? Será preciso baixar de novo para usá-lo.', { kind: 'warning', okLabel: 'Sim', cancelLabel: 'Não' }))) {
    await invoke('delete_model', { id: del }).catch((err) => toast(String(err), true));
    await renderModels();
    refreshStatus();
  }
});

listen('model-progress', ({ payload: p }) => {
  const bar = $(`#prog-${p.id}`);
  if (bar) bar.value = p.done;
  const state = $(`#state-${p.id}`);
  if (state) state.textContent = `${gb(p.done)} de ${gb(p.total)} (${Math.floor((p.done * 100) / p.total)}%)`;
});

form.addEventListener('submit', (e) => {
  e.preventDefault();
  busy(e.submitter, async () => {
    const f = form.elements;
    const settings = {
      ...loaded,
      llm_provider: f.llm_provider.value,
      local_model: f.local_model?.value || loaded.local_model,
      llm_threads: Number(f.llm_threads.value) || 0,
      tts_backend: f.tts_backend.value,
      voice_terms_accepted: f.voice_terms_accepted?.checked ?? loaded.voice_terms_accepted,
      anthropic_api_key: f.anthropic_api_key.value,
      model: f.model.value,
      level: f.level.value,
      translation_language_name: f.translation_language_name.value,
      translation_langs: f.translation_langs.value.split(',').map((x) => x.trim()).filter(Boolean),
      voicevox_url: f.voicevox_url.value,
      speaker: Number(f.speaker.value),
      speed: Number(f.speed.value),
      anki_connect_url: f.anki_connect_url.value,
      deck: f.deck.value,
    };
    await invoke('save_settings', { settings });
    loaded = settings;
    toast('Configurações salvas.');
    refreshStatus();
  });
});

/** Fills the voice list from the downloaded packs (embedded) or the VOICEVOX engine (HTTP). */
async function loadSpeakers() {
  const sel = $('#speakerSelect');
  const current = sel.value || String(loaded.speaker);
  const speakers = await invoke('speakers');
  sel.innerHTML = '';
  for (const sp of speakers) for (const st of sp.styles) sel.add(new Option(`${sp.name} (${st.name})`, st.id));
  if (![...sel.options].some((o) => o.value === current)) sel.add(new Option(`Voz ${current} (não instalada)`, current));
  sel.value = current;
  return sel.options.length;
}

$('#loadSpeakers').addEventListener('click', (e) =>
  busy(e.target, async () => {
    const n = await loadSpeakers();
    toast(`${n} vozes na lista. Salve para aplicar.`);
  }),
);

// ---------- voz embutida ----------
function showBackend() {
  const embedded = form.elements.tts_backend.value === 'embedded';
  $('#embeddedBox').classList.toggle('hidden', !embedded);
  $('#voicevoxUrlBox').classList.toggle('hidden', embedded);
}
$('#ttsBackend').addEventListener('change', () => {
  showBackend();
  // The voice list comes from a different place per backend; save first so it reads the right one.
  saveSettingsNow({ tts_backend: form.elements.tts_backend.value }).then(() => loadSpeakers().catch((e) => toast(String(e), true)));
});

/** Persists some fields right away (the backend reads them, e.g. before a download). */
async function saveSettingsNow(fields) {
  loaded = { ...loaded, ...fields };
  await invoke('save_settings', { settings: loaded });
  refreshStatus();
}

/** Packs with a download running in this window. */
const voiceDownloading = new Set();

async function renderVoices() {
  const st = await invoke('voice_status');
  if (!st.available) {
    $('#embeddedBox').innerHTML = '<p class="hint">A voz embutida não está disponível nesta plataforma. Use o programa VOICEVOX (HTTP).</p>';
    return;
  }
  const mb = (b) => Math.round(b / 1e6) + ' MB';
  $('#voicePacks').innerHTML = st.packs
    .map((p) => {
      const state = p.downloaded ? 'baixado' : p.partial_bytes ? `${Math.floor((p.partial_bytes * 100) / p.size)}% baixado` : '';
      const busyNow = voiceDownloading.has(p.file);
      const action = p.downloaded
        ? `<button type="button" class="secondary" data-delete-voice="${p.file}">Apagar</button>`
        : `<button type="button" data-download-voice="${p.file}"${busyNow || !st.terms_accepted ? ' disabled' : ''}
             title="${st.terms_accepted ? '' : 'Aceite os termos acima'}">${p.partial_bytes ? 'Continuar' : 'Baixar'}</button>`;
      const bar = busyNow ? `<progress id="vprog-${p.file.replace('.', '_')}" max="1" value="0"></progress>` : '';
      return `<li class="${p.downloaded ? 'active' : ''}">
        <span></span>
        <div><strong class="jp">${p.characters.map(escapeHtml).join('、')}</strong>
          <div class="meta">${escapeHtml(p.file)} · ${mb(p.size)}<span id="vstate-${p.file.replace('.', '_')}">${state ? ' · ' + state : ''}</span></div></div>
        <div class="actions">${action}</div>
        ${bar}
      </li>`;
    })
    .join('');
  $('#creditsBox').classList.toggle('hidden', !st.credits.length);
  $('#credits').innerHTML = st.credits
    .map(
      (c) => `<li><span class="jp">${escapeHtml(c.credit)}</span> · <a href="${escapeHtml(c.terms_url)}" target="_blank">termos</a>${
        c.note ? ` · <span class="note">${escapeHtml(c.note)}</span>` : ''
      }</li>`,
    )
    .join('');
}

$('#voiceTermsAccepted').addEventListener('change', async (e) => {
  await saveSettingsNow({ voice_terms_accepted: e.target.checked });
  renderVoices();
});

$('#voicePacks').addEventListener('click', async (e) => {
  const { downloadVoice: dl, deleteVoice: del } = e.target.dataset;
  if (dl) {
    voiceDownloading.add(dl);
    await renderVoices();
    try {
      await invoke('download_voice_pack', { file: dl });
      toast('Vozes instaladas.');
    } catch (err) {
      toast(String(err), true);
    } finally {
      voiceDownloading.delete(dl);
      await renderVoices();
      loadSpeakers().catch(() => {});
      refreshStatus();
    }
  } else if (del && (await dialog.ask('Apagar estas vozes do aparelho?', { kind: 'warning', okLabel: 'Sim', cancelLabel: 'Não' }))) {
    await invoke('delete_voice_pack', { file: del }).catch((err) => toast(String(err), true));
    await renderVoices();
    loadSpeakers().catch(() => {});
    refreshStatus();
  }
});

listen('voice-progress', ({ payload: p }) => {
  const id = p.pack.replace('.', '_');
  const bar = $(`#vprog-${id}`);
  if (bar) {
    bar.max = p.total;
    bar.value = p.done;
  }
  const state = $(`#vstate-${id}`);
  const what = p.item === p.pack ? '' : `${p.item}: `;
  if (state) state.textContent = ` · ${what}${(p.done / 1e6).toFixed(1)} de ${(p.total / 1e6).toFixed(1)} MB`;
});

const log = $('#importLog');
listen('import-log', (e) => {
  log.classList.remove('hidden');
  log.textContent += e.payload + '\n';
  log.scrollTop = log.scrollHeight;
});

$('#importTatoeba').addEventListener('click', (e) =>
  busy(e.target, async () => {
    log.textContent = '';
    await invoke('import_tatoeba');
    toast('Tatoeba importado.');
    refreshStatus();
  }),
);

$('#importDict').addEventListener('click', (e) =>
  busy(e.target, async () => {
    const n = await invoke('import_dictionary');
    toast(`${n.toLocaleString()} entradas do JMdict importadas.`);
    refreshStatus();
  }),
);

// ---------- primeiros passos ----------
async function renderSetup() {
  const s = await invoke('status');
  const done = { tatoeba: s.tatoeba_indexed > 0, dict: s.dictionary_loaded, voice: s.voice_ready, llm: s.llm_ready };
  for (const [step, ok] of Object.entries(done)) $(`#setup [data-step=${step}]`).classList.toggle('done', ok);
  const model = (await invoke('models')).find((m) => m.id === loaded.local_model);
  $('#setupModelSize').textContent = model ? `${model.name}: ${gb(model.size)}, melhor baixar no Wi-Fi.` : '';
}

function openSetup() {
  $('#setup').classList.remove('hidden');
  renderSetup().catch((e) => toast(String(e), true));
}

/** Leaves the checklist for a settings section. */
function goToSettings(section) {
  closeSetup();
  document.querySelector('[data-tab=config]').click();
  $(section).scrollIntoView({ behavior: 'smooth', block: 'start' });
}

function closeSetup() {
  $('#setup').classList.add('hidden');
  if (!loaded.setup_done) saveSettingsNow({ setup_done: true }).catch(() => {});
}

listen('import-log', (e) => {
  $('#setup [data-step=tatoeba] .step-log').textContent = e.payload;
});

$('#setup').addEventListener('click', (e) => {
  const act = e.target.dataset.act;
  if (act === 'tatoeba')
    busy(e.target, async () => {
      await invoke('import_tatoeba');
      $('#setup [data-step=tatoeba] .step-log').textContent = '';
      await renderSetup();
      refreshStatus();
    });
  else if (act === 'dict')
    busy(e.target, async () => {
      await invoke('import_dictionary');
      await renderSetup();
      refreshStatus();
    });
  else if (act === 'voice') goToSettings('#audioBox');
  else if (act === 'llm') goToSettings('#iaBox');
});
$('#setupClose').addEventListener('click', closeSetup);
$('#openSetup').addEventListener('click', openSetup);

// External links open in the system browser (the webview doesn't open new windows).
document.addEventListener('click', (e) => {
  const a = e.target.closest('a[target=_blank]');
  if (!a || !window.__TAURI__.opener) return;
  e.preventDefault();
  window.__TAURI__.opener.openUrl(a.href).catch((err) => toast(String(err), true));
});

// ---------- sobre ----------
$('#openLicenses').addEventListener('click', () => {
  // Loaded on demand: the page is large (every license text).
  if (!$('#licensesFrame').src) $('#licensesFrame').src = 'licenses.html';
  $('#licenses').classList.remove('hidden');
});
$('#closeLicenses').addEventListener('click', () => $('#licenses').classList.add('hidden'));
window.__TAURI__.app
  ?.getVersion()
  .then((v) => ($('#appVersion').textContent = 'versão ' + v))
  .catch(() => {});

// ---------- init ----------
loadSettings()
  .catch((e) => toast(String(e), true))
  .then(() => {
    renderDecks();
    if (!loaded.setup_done) openSetup();
  });
refreshStatus();
renderSelected();
