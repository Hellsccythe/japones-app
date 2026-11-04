import { test } from 'node:test';
import assert from 'node:assert/strict';
import { setupSummary, pendingItems, overallProgress } from './install.js';

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
