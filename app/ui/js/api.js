// Tauri bridge and small UI helpers shared by every tab.

export const { invoke } = window.__TAURI__.core;
export const { listen } = window.__TAURI__.event;
export const dialog = window.__TAURI__.dialog;

export const $ = (s) => document.querySelector(s);

// AnkiConnect and the VOICEVOX program only exist on the PC.
export const isAndroid = /Android/i.test(navigator.userAgent);

let toastTimer;
export function toast(msg, isError = false) {
  const t = $('#toast');
  t.textContent = msg;
  t.className = 'toast' + (isError ? ' error' : '');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add('hidden'), isError ? 7000 : 3500);
}

/** Runs `fn` with the button disabled; shows errors as a toast. */
export async function busy(btn, fn) {
  btn.disabled = true;
  btn.classList.add('busy');
  try {
    return await fn();
  } catch (e) {
    toast(String(e), true);
  } finally {
    btn.disabled = false;
    btn.classList.remove('busy');
  }
}

export function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
}

export const gb = (bytes) => (bytes / 1e9).toLocaleString('pt-BR', { maximumFractionDigits: 1, minimumFractionDigits: 1 }) + ' GB';

let currentAudio;
export async function play(text, btn) {
  await busy(btn, async () => {
    const b64 = await invoke('speak', { text });
    currentAudio?.pause();
    currentAudio = new Audio('data:audio/wav;base64,' + b64);
    await currentAudio.play();
  });
}

export const stopAudio = () => currentAudio?.pause();
