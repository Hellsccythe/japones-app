import { test } from 'node:test';
import assert from 'node:assert/strict';
import { setupSummary, pendingItems, overallProgress, planFor, initialAiChoice } from './install.js';
import { size } from './render.js';

const rec = {
  model_id: 'gemma-4-e2b',
  model_name: 'Gemma 4 E2B',
  items: [
    { id: 'tatoeba', bytes: 36e6, done: false },
    { id: 'dict', bytes: 12e6, done: false },
    { id: 'voice', bytes: 88e6, done: false },
    { id: 'model', bytes: 3_349_516_256, done: false },
  ],
  total_bytes: 36e6 + 12e6 + 88e6 + 3_349_516_256,
};

test('summary lists what is missing and the total', () =>
  assert.equal(setupSummary(rec), 'Frases, dicionário, voz e IA (Gemma 4 E2B): 3,5 GB'));

test('summary skips what is installed', () => {
  const r = { ...rec, items: rec.items.map((i) => ({ ...i, done: i.id !== 'model' })), total_bytes: 3_349_516_256 };
  assert.equal(setupSummary(r), 'IA (Gemma 4 E2B): 3,3 GB');
});

test('summary when everything is there', () => {
  const r = { ...rec, items: rec.items.map((i) => ({ ...i, done: true })), total_bytes: 0 };
  assert.equal(setupSummary(r), 'Tudo instalado.');
});

test('pending items keep the install order', () => {
  const r = { ...rec, items: [rec.items[3], { ...rec.items[0], done: true }, rec.items[1]] };
  assert.deepEqual(pendingItems(r).map((i) => i.id), ['dict', 'model']);
});

test('overall progress adds finished items and the current one', () => {
  const items = [{ id: 'a', bytes: 100 }, { id: 'b', bytes: 300 }];
  assert.equal(overallProgress(items, new Set(), null, 0), 0);
  assert.equal(overallProgress(items, new Set(['a']), 'b', 150), 0.625);
  assert.equal(overallProgress(items, new Set(['a', 'b']), null, 0), 1);
  // A current item reporting more than its size doesn't go past it.
  assert.equal(overallProgress(items, new Set(), 'a', 1000), 0.25);
  assert.equal(overallProgress([], new Set(), null, 0), 1);
});

test('only the local AI choice keeps the model in the plan', () => {
  assert.ok(planFor(rec, 'local').items.some((i) => i.id === 'model'));
  for (const choice of ['openrouter', 'claude', 'none', null])
    assert.ok(!planFor(rec, choice).items.some((i) => i.id === 'model'), String(choice));
});

test('without the model the summary is in MB', () =>
  assert.equal(setupSummary(planFor(rec, 'none')), 'Frases, dicionário e voz: 136 MB'));

test('the PC suggests the local AI; the phone asks', () => {
  assert.equal(initialAiChoice({ provider: 'local', modelDone: false, android: false }), 'local');
  assert.equal(initialAiChoice({ provider: 'local', modelDone: false, android: true }), null);
});

test('a choice already made comes back when the setup reopens', () => {
  assert.equal(initialAiChoice({ provider: 'openrouter', modelDone: false, android: true }), 'openrouter');
  assert.equal(initialAiChoice({ provider: 'claude', modelDone: false, android: false }), 'claude');
  assert.equal(initialAiChoice({ provider: 'local', modelDone: true, android: true }), 'local');
  assert.equal(initialAiChoice({ provider: 'off', modelDone: false, android: false }), 'none');
});

test('sizes under 1 GB are in MB', () => {
  assert.equal(size(36e6), '36 MB');
  assert.equal(size(12_400_000), '12 MB');
  assert.equal(size(3_349_516_256), '3,3 GB');
  assert.equal(size(0), '0 MB');
});
