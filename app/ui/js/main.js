// Entry point: tabs, external links and start-up.

import { $, toast, isAndroid } from './api.js';
import { icon } from './icons.js';
import { refreshStatus } from './status.js';
import * as frases from './frases.js';
import * as estudar from './estudar.js';
import * as texto from './texto.js';
import * as config from './config.js';
import * as setup from './setup.js';
import * as insets from './insets.js';

document.documentElement.classList.toggle('android', isAndroid);
insets.init();

// Icons named in the markup (data-icon) go in front of the label.
document.querySelectorAll('[data-icon]').forEach((el) => el.insertAdjacentHTML('afterbegin', icon(el.dataset.icon)));

document.querySelectorAll('.tab').forEach((b) =>
  b.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach((x) => x.classList.toggle('active', x === b));
    document.querySelectorAll('.panel').forEach((p) => p.classList.toggle('active', p.id === 'tab-' + b.dataset.tab));
  }),
);

// External links open in the system browser (the webview doesn't open new windows).
document.addEventListener('click', (e) => {
  const a = e.target.closest('a[target=_blank]');
  if (!a || !window.__TAURI__.opener) return;
  e.preventDefault();
  window.__TAURI__.opener.openUrl(a.href).catch((err) => toast(String(err), true));
});

for (const m of [frases, estudar, texto, config, setup]) m.init();

$('#statusDot').addEventListener('click', (e) => config.openSection(e.currentTarget.dataset.section || 'sobre'));

config
  .loadSettings()
  .catch((e) => toast(String(e), true))
  .then(() => {
    estudar.renderDecks();
    if (!config.getLoaded().setup_done) setup.openSetup();
  });
refreshStatus();
$('#word').focus();
