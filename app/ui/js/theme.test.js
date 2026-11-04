import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveTheme } from './theme.js';

test('system follows the OS', () => {
  assert.equal(resolveTheme('system', true), 'dark');
  assert.equal(resolveTheme('system', false), 'light');
});

test('explicit choice wins', () => {
  assert.equal(resolveTheme('light', true), 'light');
  assert.equal(resolveTheme('dark', false), 'dark');
});

test('unknown value means system', () => assert.equal(resolveTheme('blue', true), 'dark'));
