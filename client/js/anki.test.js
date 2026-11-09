import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ankiMessage } from './anki.js';

test('known codes get a friendly message and the install help', () => {
  assert.deepEqual(ankiMessage('anki:not_running'), { text: 'O Anki não está aberto. Abra o Anki e tente de novo.', help: true });
  assert.deepEqual(ankiMessage('anki:no_addon'), { text: 'Falta o add-on AnkiConnect no Anki.', help: true });
  assert.equal(ankiMessage('anki:incompatible:5').text, 'O AnkiConnect instalado (versão 5) é antigo. Atualize o add-on.');
});

test('any other error passes through without help', () => {
  assert.deepEqual(ankiMessage('Error: disco cheio'), { text: 'Error: disco cheio', help: false });
});
