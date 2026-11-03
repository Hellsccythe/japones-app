// "Texto" tab: furigana, audio and PDF for any text.

import { $, invoke, dialog, toast, busy, play } from './api.js';

export function init() {
  $('#showFurigana').addEventListener('click', (e) =>
    busy(e.currentTarget, async () => {
      const lines = $('#text').value.split('\n').filter((l) => l.trim());
      const out = await Promise.all(lines.map((l) => invoke('furigana', { text: l.trim() })));
      $('#textOut').innerHTML = out
        .map((f, i) => `<p><button class="icon play" data-i="${i}" title="Ouvir">▶</button><span>${f.html}</span></p>`)
        .join('');
      $('#textOut').dataset.lines = JSON.stringify(lines);
    }),
  );

  $('#textOut').addEventListener('click', (e) => {
    const b = e.target.closest('.play');
    if (!b) return;
    const lines = JSON.parse($('#textOut').dataset.lines || '[]');
    play(lines[b.dataset.i], b);
  });

  $('#playText').addEventListener('click', (e) => {
    const text = $('#text').value.trim();
    if (text) play(text, e.currentTarget);
  });

  $('#hideRt').addEventListener('change', (e) => $('#textOut').classList.toggle('hide-rt', e.target.checked));

  $('#textPdf').addEventListener('click', (e) => {
    const text = $('#text').value.trim();
    if (!text) return toast('Digite um texto.', true);
    busy(e.currentTarget, async () => {
      const path = await dialog.save({ defaultPath: 'texto.pdf', filters: [{ name: 'PDF', extensions: ['pdf'] }] });
      if (!path) return;
      await invoke('export_text_pdf', { path, title: 'Texto', text, furigana: $('#textPdfFurigana').checked });
      toast('PDF salvo.');
    });
  });
}
