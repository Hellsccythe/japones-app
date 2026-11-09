// "Instalar o recomendado": texts and the single progress bar (pure: no Tauri here).

import { size } from './render.js';

const NAMES = { tatoeba: 'Frases', dict: 'dicionário', voice: 'voz' };
const ORDER = ['tatoeba', 'dict', 'voice', 'model'];

const name = (item, rec) => (item.id === 'model' ? `IA (${rec.model_name})` : NAMES[item.id] ?? item.id);

/** Items still to download, in the order they're installed. */
export function pendingItems(rec) {
  return ORDER.flatMap((id) => rec.items.filter((i) => i.id === id && !i.done));
}

/** "Frases, dicionário, voz e IA (Gemma 4 E2B): 3,5 GB" for what is missing. */
export function setupSummary(rec) {
  const items = pendingItems(rec);
  if (!items.length) return 'Tudo instalado.';
  const names = items.map((i) => name(i, rec));
  names[0] = names[0][0].toUpperCase() + names[0].slice(1);
  const list = names.length > 1 ? `${names.slice(0, -1).join(', ')} e ${names.at(-1)}` : names[0];
  return `${list}: ${size(items.reduce((n, i) => n + i.bytes, 0))}`;
}

/** Fraction 0..1 over all `items`: the finished ones plus `doneBytes` of `currentId`. */
export function overallProgress(items, finished, currentId, doneBytes) {
  const total = items.reduce((n, i) => n + i.bytes, 0);
  if (!total) return 1;
  let have = 0;
  for (const i of items) {
    if (finished.has(i.id)) have += i.bytes;
    else if (i.id === currentId) have += Math.min(doneBytes, i.bytes);
  }
  return have / total;
}

/** The plan for an AI choice ('local' | 'openrouter' | 'claude' | 'none' | null): only 'local' downloads the model. */
export function planFor(rec, choice) {
  return { ...rec, items: rec.items.filter((i) => i.id !== 'model' || choice === 'local') };
}

/**
 * The AI option selected when "Primeiros passos" opens: an online AI or "off" already chosen
 * comes back, a model on disk means local; otherwise the PC suggests the local AI and the
 * phone selects nothing, so the person has to answer.
 */
export function initialAiChoice({ provider, modelDone, android }) {
  if (provider === 'claude' || provider === 'openrouter') return provider;
  if (provider === 'off') return 'none';
  if (modelDone) return 'local';
  return android ? null : 'local';
}
