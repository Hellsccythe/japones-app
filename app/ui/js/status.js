// Traffic-light status in the navigation, plus dots on the settings sections.

import { $, invoke } from './api.js';
import { health } from './health.js';

const COLORS = { ok: 'ok', warn: 'warn', bad: 'bad' };

/** Paints the indicator from `status()`; its click opens `dataset.section`. */
export function setStatus(s) {
  const h = health(s);
  const dot = $('#statusDot');
  dot.className = 'status-dot ' + COLORS[h.level];
  dot.dataset.section = h.section ?? '';
  dot.title = [
    s.tatoeba_indexed ? `${s.tatoeba_indexed.toLocaleString('pt-BR')} frases do Tatoeba` : 'Tatoeba não baixado',
    s.dictionary_loaded ? 'dicionário ok' : 'sem dicionário',
    s.llm_ready ? `IA: ${s.llm_name}` : `IA: ${s.llm_name} (não configurada)`,
    s.voice_ready ? `voz: ${s.voice_name}` : 'voz não instalada',
  ].join('\n');
  for (const [sec, missing] of Object.entries(h.missing)) {
    $(`.cfg-nav [data-sec=${sec}] .dot`)?.classList.toggle('on', missing);
  }
}

export async function refreshStatus() {
  try {
    setStatus(await invoke('status'));
  } catch (e) {
    $('#statusDot').title = String(e);
  }
}
