// First run ("Primeiros passos": install the recommended set with one progress bar)
// and the quick setup modal opened when something is missing.

import { $, invoke, listen, toast, busy, gb, escapeHtml, isAndroid } from './api.js';
import { size } from './render.js';
import { refreshStatus } from './status.js';
import { getLoaded, saveSettingsNow, openSection, renderModels } from './config.js';
import { setupSummary, pendingItems, overallProgress, planFor, initialAiChoice } from './install.js';

const PAUSED = 'download:paused';
const isPaused = (err) => String(err).includes(PAUSED);

const ITEM_TEXT = {
  tatoeba: 'Frases do Tatoeba',
  dict: 'Dicionário JMdict',
  voice: 'Voz (pacote 0.vvm)',
};
const itemText = (item, rec) => (item.id === 'model' ? `IA local: ${rec.model_name}` : ITEM_TEXT[item.id]);

/** Last `recommended_setup()`; the model is in the plan only when the local AI is chosen. */
let rec = null;
/** "Como você quer usar a IA?": 'local' | 'openrouter' | 'claude' | 'none' | null (not answered); undefined = not set up yet. */
let aiChoice;
/** Online AIs whose key passed "Testar e usar" in this setup. */
const keyOk = new Set();
const ONLINE = ['openrouter', 'claude'];
const plan = () => (rec ? planFor(rec, aiChoice) : null);
const needsKey = () => ONLINE.includes(aiChoice) && !keyOk.has(aiChoice);

// ---------- progress of the install ----------
const run = { active: false, paused: false, items: [], finished: new Set(), current: null, done: 0, sub: {} };

function paintProgress(text) {
  $('#setupBar').value = overallProgress(run.items, run.finished, run.current, run.done);
  if (text !== undefined) $('#setupNow').textContent = text;
}

/** Downloads one item; resolves when it's installed. */
async function installItem(item) {
  run.current = item.id;
  run.done = 0;
  run.sub = {};
  paintProgress(`Baixando ${itemText(item, rec).toLowerCase()}…`);
  if (item.id === 'tatoeba') await invoke('import_tatoeba');
  else if (item.id === 'dict') await invoke('import_dictionary');
  else if (item.id === 'voice') {
    if (!getLoaded().voice_terms_accepted) await saveSettingsNow({ voice_terms_accepted: true });
    await invoke('download_voice_pack', { file: '0.vvm' });
    // The default voice is in 0.vvm; keep the one chosen if it's there too.
  } else if (item.id === 'model') {
    await invoke('download_model', { id: rec.model_id });
    await saveSettingsNow({ llm_provider: 'local', local_model: rec.model_id });
    renderModels().catch(() => {});
  }
  run.finished.add(item.id);
  run.current = null;
  paintProgress();
}

/** The key field and result line of an online AI option. */
function keyBox(provider) {
  const box = $(`#setupAi .ai-key[data-for=${provider}]`);
  return { input: box.querySelector('input'), result: box.querySelector('[data-result]') };
}

/** Saves the AI choice that needs no download: "Agora não" turns the AI off, a local model already on disk is used. */
async function applyAiChoice() {
  const s = getLoaded();
  if (aiChoice === 'none' && s.llm_provider !== 'off') await saveSettingsNow({ llm_provider: 'off' });
  if (aiChoice === 'local' && s.llm_provider !== 'local') await saveSettingsNow({ llm_provider: 'local', local_model: rec.model_id });
}

async function install() {
  if (aiChoice == null) {
    toast('Escolha como usar a IA (ou “Agora não”).', true);
    $('#setupAi input').focus();
    return;
  }
  if (needsKey()) {
    const { input, result } = keyBox(aiChoice);
    if (!(await testAndUse(aiChoice, input.value, result))) return input.focus();
    keyOk.add(aiChoice);
  }
  const p = plan();
  const todo = pendingItems(p);
  if (todo.some((i) => i.id === 'voice') && !getLoaded().voice_terms_accepted && !$('#setupTermsOk').checked) {
    toast('Aceite os termos das vozes para instalar a voz.', true);
    $('#setupTermsOk').focus();
    return;
  }
  Object.assign(run, { active: true, paused: false, items: p.items.filter((i) => !i.done), finished: new Set(), current: null });
  $('#setupProgress').classList.remove('hidden');
  $('#setupInstall').classList.add('hidden');
  $('#setupPause').classList.remove('hidden');
  try {
    for (const item of todo) {
      if (run.paused) break;
      await installItem(item);
    }
    if (run.paused) paintProgress('Pausado. O download continua de onde parou.');
    else {
      await applyAiChoice();
      paintProgress('Pronto! Tudo instalado.');
      toast('Instalação concluída.');
    }
  } catch (err) {
    if (isPaused(err) || run.paused) paintProgress('Pausado. O download continua de onde parou.');
    else {
      paintProgress('');
      $('#setupNow').innerHTML = `<span class="bad">Erro: ${escapeHtml(String(err))}</span>`;
    }
  } finally {
    run.active = false;
    $('#setupPause').classList.add('hidden');
    $('#setupInstall').classList.remove('hidden');
    refreshStatus();
    await renderSetup().catch(() => {});
    const label = $('#setupInstall span');
    if (run.paused) label.textContent = 'Continuar';
    else if (pendingItems(plan()).length) label.textContent = 'Tentar de novo';
  }
}

function pause() {
  run.paused = true;
  $('#setupPause').disabled = true;
  paintProgress('Pausando…');
  // Model and voice stop right away; the corpus and the dictionary finish the item they're on.
  invoke('cancel_downloads').catch(() => {});
}

// ---------- the overlay ----------
/** Paints the plan for the current AI choice (no Tauri calls). */
function paintPlan() {
  const p = plan();
  $('#setupSummary').textContent = setupSummary(p);
  $('#setupItems').innerHTML = p.items
    .map(
      (i) => `<li class="${i.done ? 'done' : ''}"><span>${escapeHtml(itemText(i, rec))}</span><small>${i.done ? 'instalado' : size(i.bytes)}</small></li>`,
    )
    .join('');
  const needsTerms = pendingItems(p).some((i) => i.id === 'voice') && !getLoaded().voice_terms_accepted;
  $('#setupTerms').classList.toggle('hidden', !needsTerms);

  const model = rec.items.find((i) => i.id === 'model');
  $('#setupAiLocal').textContent = model?.done
    ? `${rec.model_name}, já baixado. Roda neste aparelho, offline.`
    : `${rec.model_name}, ${size(model?.bytes ?? 0)}. Roda neste aparelho, offline.`;
  for (const radio of document.querySelectorAll('#setupAi input[type=radio]')) radio.checked = radio.value === aiChoice;
  for (const box of document.querySelectorAll('#setupAi .ai-key')) box.classList.toggle('hidden', box.dataset.for !== aiChoice);

  // Unanswered on the phone: the button waits for a choice.
  const provider = getLoaded().llm_provider;
  const changesAi = needsKey() || (aiChoice === 'none' && provider !== 'off') || (aiChoice === 'local' && provider !== 'local');
  $('#setupInstall').disabled = aiChoice == null || (!pendingItems(p).length && !changesAi);
}

async function renderSetup() {
  rec = await invoke('recommended_setup');
  if (aiChoice === undefined) {
    const modelDone = !!rec.items.find((i) => i.id === 'model')?.done;
    aiChoice = initialAiChoice({ provider: getLoaded().llm_provider, modelDone, android: isAndroid });
    // An online AI already set up counts as tested.
    if (ONLINE.includes(aiChoice)) keyOk.add(aiChoice);
  }
  paintPlan();
  $('#setupPause').disabled = false;

  // The manual checklist ("Escolher eu mesmo").
  const s = await invoke('status');
  const done = { tatoeba: s.tatoeba_indexed > 0, dict: s.dictionary_loaded, voice: s.voice_ready, llm: s.llm_ready };
  for (const [step, ok] of Object.entries(done)) $(`#setup [data-step=${step}]`).classList.toggle('done', ok);
  const model = (await invoke('models')).find((m) => m.id === (getLoaded().local_model || rec.model_id));
  $('#setupModelSize').textContent = model ? `${model.name}: ${gb(model.size)}.` : '';
}

export function openSetup() {
  aiChoice = undefined;
  keyOk.clear();
  $('#setup').classList.remove('hidden');
  renderSetup().catch((e) => toast(String(e), true));
}

function closeSetup() {
  if (run.active) pause();
  $('#setup').classList.add('hidden');
  if (!getLoaded().setup_done) saveSettingsNow({ setup_done: true }).catch(() => {});
}

/** Leaves the checklist for a settings section. */
function goToSettings(section) {
  closeSetup();
  openSection(section);
}

const PROVIDERS = {
  claude: { name: 'o Claude', test: 'test_claude', field: 'anthropic_api_key' },
  openrouter: { name: 'o OpenRouter', test: 'test_openrouter', field: 'openrouter_api_key' },
};

/** Checks the key of an online AI; on success it's saved and becomes the AI. */
async function testAndUse(provider, key, out) {
  const p = PROVIDERS[provider];
  out.className = 'hint';
  out.textContent = 'Testando…';
  try {
    await invoke(p.test, { key });
    await saveSettingsNow({ llm_provider: provider, [p.field]: key.trim() || getLoaded()[p.field] });
    out.textContent = `Chave ok. O app vai usar ${p.name}.`;
    out.className = 'hint ok';
    return true;
  } catch (err) {
    out.textContent = String(err);
    out.className = 'hint bad';
    return false;
  }
}

// ---------- quick setup modal ----------
const QUICK = {
  ia: { title: 'Configurar a IA', status: (s) => s.llm_ready },
  voz: { title: 'Instalar uma voz', status: (s) => s.voice_ready },
  frases: { title: 'Baixar as frases', status: (s) => s.tatoeba_indexed > 0 },
  dict: { title: 'Baixar o dicionário', status: (s) => s.dictionary_loaded },
};

function closeQuick() {
  $('#quick').classList.add('hidden');
}

/** Quick setup for one missing piece: a line of explanation, the size and the action. */
export async function openQuickSetup(item) {
  const q = QUICK[item];
  $('#quickTitle').textContent = q.title;
  $('#quickBody').innerHTML = '<p class="hint">Carregando…</p>';
  $('#quick').classList.remove('hidden');
  try {
    rec = await invoke('recommended_setup');
  } catch (err) {
    $('#quickBody').innerHTML = `<p class="hint bad">${escapeHtml(String(err))}</p>`;
    return;
  }
  const bytesOf = (id) => rec.items.find((i) => i.id === id)?.bytes ?? 0;
  // The AI may just be off (or set to Claude) with the model already on disk.
  const modelDone = rec.items.find((i) => i.id === 'model')?.done;
  const bar = '<div class="quick-progress hidden"><progress max="1" value="0"></progress><small class="hint"></small></div>';
  const body = {
    ia: () => `
      <p>A IA gera frases quando o corpus tem poucas e traduz o vocabulário.</p>
      ${getLoaded().llm_provider === 'off' ? '<p class="hint">A IA está desligada; escolher uma opção abaixo liga de novo.</p>' : ''}
      <div class="quick-opt">
        ${
          modelDone
            ? `<div><strong>Usar ${escapeHtml(rec.model_name)}</strong><small>Já está baixado neste aparelho; roda offline.</small></div>
        <button type="button" data-q="model">Usar</button>`
            : `<div><strong>Baixar ${escapeHtml(rec.model_name)}</strong><small>Roda neste aparelho, offline. ${size(bytesOf('model'))}.</small></div>
        <button type="button" data-q="model">Baixar</button>`
        }
      </div>
      ${bar}
      <div class="quick-opt col">
        <div><strong>Usar o Claude</strong><small>Online, com uma chave da API da Anthropic.
          <a href="https://console.anthropic.com/settings/keys" target="_blank">Criar uma chave</a></small></div>
        <div class="row"><input id="quickKey" type="password" placeholder="sk-ant-…" aria-label="Chave da API do Claude" autocomplete="off">
          <button type="button" data-q="claude" class="ghost">Testar e usar</button></div>
        <small id="quickKeyResult" class="hint"></small>
      </div>
      <div class="quick-opt col">
        <div><strong>Usar o OpenRouter</strong><small>Online, vários modelos (alguns gratuitos) com uma chave.
          <a href="https://openrouter.ai/settings/keys" target="_blank">Criar uma chave</a></small></div>
        <div class="row"><input id="quickOrKey" type="password" placeholder="sk-or-…" aria-label="Chave do OpenRouter" autocomplete="off">
          <button type="button" data-q="openrouter" class="ghost">Testar e usar</button></div>
        <small id="quickOrKeyResult" class="hint"></small>
      </div>`,
    voz: () =>
      rec.items.some((i) => i.id === 'voice')
        ? `<p>Para ouvir as frases e pôr áudio nos cards. ${size(bytesOf('voice'))}.</p>
          <div class="terms"><p>As vozes VOICEVOX são livres para uso comercial e não comercial; ao publicar um áudio, dê o crédito da voz.
            <a href="https://github.com/VOICEVOX/voicevox_vvm#voicevox-音声モデル-利用規約" target="_blank">ler termos</a></p>
            <label class="sw-row"><span>Li e aceito os termos</span><span class="switch"><input type="checkbox" id="quickTerms"${
              getLoaded().voice_terms_accepted ? ' checked' : ''
            }><span></span></span></label></div>
          <div class="row"><button type="button" data-q="voice">Baixar a voz</button></div>${bar}`
        : `<p>A voz embutida não está disponível neste aparelho. No PC, dá para usar o programa VOICEVOX aberto.</p>
          <div class="row"><button type="button" data-q="settings-voz" class="ghost">Abrir Configurações › Voz</button></div>`,
    frases: () => `<p>Frases reais com tradução, para buscar exemplos de qualquer palavra. ${size(bytesOf('tatoeba'))}.</p>
      <div class="row"><button type="button" data-q="tatoeba">Baixar as frases</button></div>${bar}`,
    dict: () => `<p>Significado e leitura das palavras. ${size(bytesOf('dict'))}.</p>
      <div class="row"><button type="button" data-q="dict">Baixar o dicionário</button></div>${bar}`,
  }[item]();
  $('#quickBody').innerHTML = body;
}

/** Runs `fn` with the modal's progress bar listening to `event`. */
async function withQuickProgress(event, fn) {
  const box = $('#quickBody .quick-progress');
  box?.classList.remove('hidden');
  const stop = await listen(event, ({ payload: p }) => {
    if (!box || !p.total) return;
    box.querySelector('progress').value = p.done / p.total;
    box.querySelector('small').textContent = `${gb(p.done)} de ${gb(p.total)}`;
  });
  try {
    await fn();
  } finally {
    stop();
  }
}

async function quickAction(btn) {
  const act = btn.dataset.q;
  if (act === 'settings-voz') {
    closeQuick();
    return openSection('voz');
  }
  if (act === 'claude') {
    return busy(btn, async () => {
      if (await testAndUse('claude', $('#quickKey').value, $('#quickKeyResult'))) setTimeout(closeQuick, 800);
    });
  }
  if (act === 'openrouter') {
    return busy(btn, async () => {
      if (await testAndUse('openrouter', $('#quickOrKey').value, $('#quickOrKeyResult'))) setTimeout(closeQuick, 800);
    });
  }
  await busy(btn, async () => {
    if (act === 'model')
      await withQuickProgress('model-progress', async () => {
        if (!rec.items.find((i) => i.id === 'model')?.done) await invoke('download_model', { id: rec.model_id });
        await saveSettingsNow({ llm_provider: 'local', local_model: rec.model_id });
        renderModels().catch(() => {});
      });
    else if (act === 'voice') {
      if (!$('#quickTerms').checked) {
        toast('Aceite os termos das vozes antes de baixar.', true);
        return;
      }
      if (!getLoaded().voice_terms_accepted) await saveSettingsNow({ voice_terms_accepted: true });
      await withQuickProgress('voice-progress', () => invoke('download_voice_pack', { file: '0.vvm' }));
    } else if (act === 'tatoeba') await invoke('import_tatoeba');
    else if (act === 'dict') await invoke('import_dictionary');
    refreshStatus();
    toast('Pronto.');
    closeQuick();
  });
}

/**
 * True when `item` ('ia' | 'voz' | 'frases' | 'dict') can be used now;
 * otherwise opens its quick setup and returns false.
 */
export async function ensureReady(item) {
  try {
    const s = await invoke('status');
    if (QUICK[item].status(s)) return true;
  } catch {
    return true; // let the action itself report the problem
  }
  openQuickSetup(item);
  return false;
}

export function init() {
  listen('import-log', (e) => {
    $('#setup [data-step=tatoeba] .step-log').textContent = e.payload;
    if (run.current === 'tatoeba') paintProgress(e.payload);
  });
  listen('model-progress', ({ payload: p }) => {
    if (run.current !== 'model') return;
    run.done = p.done;
    paintProgress(`IA: ${gb(p.done)} de ${gb(p.total)}`);
  });
  listen('voice-progress', ({ payload: p }) => {
    if (run.current !== 'voice') return;
    run.sub[p.item] = p.done;
    run.done = Object.values(run.sub).reduce((a, b) => a + b, 0);
    paintProgress(`Voz: ${p.item === p.pack ? p.pack : p.item} ${(p.done / 1e6).toFixed(1)} de ${(p.total / 1e6).toFixed(1)} MB`);
  });

  $('#setupInstall').addEventListener('click', () => install());
  $('#setupPause').addEventListener('click', pause);
  $('#setupManual').addEventListener('click', () => $('#setupSteps').classList.toggle('hidden'));
  $('#setupAi').addEventListener('change', (e) => {
    if (e.target.name !== 'setupAi' || run.active) return;
    aiChoice = e.target.value;
    paintPlan();
    if (needsKey()) keyBox(aiChoice).input.focus();
  });
  $('#setupAi').addEventListener('click', (e) => {
    const btn = e.target.closest('[data-test]');
    if (!btn) return;
    const provider = btn.dataset.test;
    const { input, result } = keyBox(provider);
    busy(btn, async () => {
      if (await testAndUse(provider, input.value, result)) {
        keyOk.add(provider);
        paintPlan();
      }
    });
  });

  $('#setupSteps').addEventListener('click', (e) => {
    const btn = e.target.closest('[data-act]');
    if (!btn) return;
    const act = btn.dataset.act;
    if (act === 'tatoeba')
      busy(btn, async () => {
        await invoke('import_tatoeba');
        $('#setup [data-step=tatoeba] .step-log').textContent = '';
        await renderSetup();
        refreshStatus();
      });
    else if (act === 'dict')
      busy(btn, async () => {
        await invoke('import_dictionary');
        await renderSetup();
        refreshStatus();
      });
    else if (act === 'voice') goToSettings('voz');
    else if (act === 'llm') goToSettings('ia');
  });
  $('#setupClose').addEventListener('click', closeSetup);
  $('#openSetup').addEventListener('click', openSetup);

  $('#quickClose').addEventListener('click', closeQuick);
  $('#quick').addEventListener('click', (e) => {
    if (e.target === e.currentTarget) return closeQuick();
    const btn = e.target.closest('[data-q]');
    if (btn) quickAction(btn);
  });
}
