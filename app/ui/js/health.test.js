import { test } from 'node:test';
import assert from 'node:assert/strict';
import { health } from './health.js';

const all = { tatoeba_indexed: 10, dictionary_loaded: true, voice_ready: true, llm_ready: true };

test('everything installed is green', () => {
  assert.deepEqual(health(all), { level: 'ok', section: null, missing: { frases: false, voz: false, ia: false } });
});

test('no sentences and no AI is red and points to Frases', () => {
  const h = health({ ...all, tatoeba_indexed: 0, llm_ready: false });
  assert.equal(h.level, 'bad');
  assert.equal(h.section, 'frases');
});

test('no sentences but AI ready is only yellow', () => {
  assert.equal(health({ ...all, tatoeba_indexed: 0 }).level, 'warn');
});

test('a missing optional piece is yellow and points to it', () => {
  const h = health({ ...all, voice_ready: false });
  assert.deepEqual([h.level, h.section, h.missing.voz], ['warn', 'voz', true]);
  assert.equal(health({ ...all, dictionary_loaded: false }).section, 'frases');
  assert.equal(health({ ...all, llm_ready: false }).section, 'ia');
});
