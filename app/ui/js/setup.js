// First-run checklist: what to download and how big it is.

import { $, invoke, listen, toast, busy, gb } from './api.js';
import { refreshStatus } from './status.js';
import { getLoaded, saveSettingsNow } from './config.js';

async function renderSetup() {
  const s = await invoke('status');
  const done = { tatoeba: s.tatoeba_indexed > 0, dict: s.dictionary_loaded, voice: s.voice_ready, llm: s.llm_ready };
  for (const [step, ok] of Object.entries(done)) $(`#setup [data-step=${step}]`).classList.toggle('done', ok);
  const model = (await invoke('models')).find((m) => m.id === getLoaded().local_model);
  $('#setupModelSize').textContent = model ? `${model.name}: ${gb(model.size)}, melhor baixar no Wi-Fi.` : '';
}

export function openSetup() {
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
  if (!getLoaded().setup_done) saveSettingsNow({ setup_done: true }).catch(() => {});
}

export function init() {
  listen('import-log', (e) => {
    $('#setup [data-step=tatoeba] .step-log').textContent = e.payload;
  });

  $('#setup').addEventListener('click', (e) => {
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
    else if (act === 'voice') goToSettings('#audioBox');
    else if (act === 'llm') goToSettings('#iaBox');
  });
  $('#setupClose').addEventListener('click', closeSetup);
  $('#openSetup').addEventListener('click', openSetup);
}
