// Corpus, dictionary, AI and voice status shown in the app's chrome.

import { $, invoke } from './api.js';

export async function refreshStatus() {
  try {
    const s = await invoke('status');
    const parts = [
      s.tatoeba_indexed ? `${s.tatoeba_indexed.toLocaleString('pt-BR')} frases do Tatoeba` : 'Tatoeba não importado',
      s.dictionary_loaded ? 'dicionário ok' : 'sem dicionário',
      s.llm_ready ? `IA: ${s.llm_name}` : `IA: ${s.llm_name} (não configurada)`,
      s.voice_ready ? `voz: ${s.voice_name}` : 'voz não instalada',
    ];
    $('#status').textContent = parts.join(' · ');
  } catch (e) {
    $('#status').textContent = String(e);
  }
}
