// "Configurações" tab: settings form, AI models, voices and data import.

import { $, invoke, listen, dialog, toast, busy, escapeHtml, gb } from './api.js';
import { refreshStatus } from './status.js';
import { applyTheme } from './theme.js';
import { applyColors } from './colors.js';

const form = () => $('#settings');
/** Last settings from the backend; saving starts from these so fields without inputs keep their values. */
let loaded = {};
export const getLoaded = () => loaded;

/** Theme, word-class colors and Japanese text size from the settings. */
export function applyAppearance(s) {
  applyTheme(s.theme);
  applyColors(s.pos_colors, s.pos_colors_on);
  document.documentElement.dataset.jpSize = s.jp_font_size || 'medium';
}

export async function loadSettings() {
  const s = await invoke('get_settings');
  loaded = s;
  applyAppearance(s);
  await Promise.all([renderModels(), renderVoices()]);
  for (const [k, v] of Object.entries(s)) {
    const el = form().elements[k];
    if (!el || el instanceof RadioNodeList) continue;
    if (k === 'speaker' && !el.options.length) el.add(new Option(`Voz ${v}`, v));
    if (el.type === 'checkbox') el.checked = v;
    else el.value = Array.isArray(v) ? v.join(', ') : v;
  }
  showProvider();
  showBackend();
  loadSpeakers().catch(() => {});
}

/** Persists some fields right away (the backend reads them, e.g. before a download). */
export async function saveSettingsNow(fields) {
  loaded = { ...loaded, ...fields };
  await invoke('save_settings', { settings: loaded });
  refreshStatus();
}

// ---------- IA ----------
function showProvider() {
  const local = form().elements.llm_provider.value === 'local';
  $('#localBox').classList.toggle('hidden', !local);
  $('#claudeBox').classList.toggle('hidden', local);
}

/** Model ids with a download running in this window. */
const downloading = new Set();

export async function renderModels() {
  const models = await invoke('models');
  const current = form().elements.local_model?.value || loaded.local_model;
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

// ---------- voz embutida ----------
function showBackend() {
  const embedded = form().elements.tts_backend.value === 'embedded';
  $('#embeddedBox').classList.toggle('hidden', !embedded);
  $('#voicevoxUrlBox').classList.toggle('hidden', embedded);
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

export function init() {
  $('#llmProvider').addEventListener('change', showProvider);

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

  form().addEventListener('submit', (e) => {
    e.preventDefault();
    busy(e.submitter, async () => {
      const f = form().elements;
      const settings = {
        ...loaded,
        theme: f.theme.value,
        pos_colors_on: f.pos_colors_on.checked,
        jp_font_size: f.jp_font_size.value,
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
      applyAppearance(settings);
      toast('Configurações salvas.');
      refreshStatus();
    });
  });

  // Appearance applies as soon as it changes; "Salvar" persists it.
  for (const name of ['theme', 'pos_colors_on', 'jp_font_size']) {
    form().elements[name].addEventListener('change', () => {
      const f = form().elements;
      applyAppearance({ ...loaded, theme: f.theme.value, pos_colors_on: f.pos_colors_on.checked, jp_font_size: f.jp_font_size.value });
    });
  }

  $('#loadSpeakers').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const n = await loadSpeakers();
      toast(`${n} vozes na lista. Salve para aplicar.`);
    }),
  );

  $('#ttsBackend').addEventListener('change', () => {
    showBackend();
    // The voice list comes from a different place per backend; save first so it reads the right one.
    saveSettingsNow({ tts_backend: form().elements.tts_backend.value }).then(() => loadSpeakers().catch((e) => toast(String(e), true)));
  });

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
    busy(e.currentTarget, async () => {
      log.textContent = '';
      await invoke('import_tatoeba');
      toast('Tatoeba importado.');
      refreshStatus();
    }),
  );

  $('#importDict').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const n = await invoke('import_dictionary');
      toast(`${n.toLocaleString('pt-BR')} entradas do JMdict importadas.`);
      refreshStatus();
    }),
  );

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
}
