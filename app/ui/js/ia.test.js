import { test } from 'node:test';
import assert from 'node:assert/strict';
import { needsAiSetup } from './ia.js';

test('AI turned off or not ready opens the quick setup', () => {
  assert.equal(needsAiSetup('ia:off'), true);
  assert.equal(needsAiSetup('ia:not_ready'), true);
  assert.equal(needsAiSetup(new Error('ia:not_ready')), true);
});

test('other errors are just shown', () => {
  assert.equal(needsAiSetup('chave inválida'), false);
  assert.equal(needsAiSetup(undefined), false);
});
