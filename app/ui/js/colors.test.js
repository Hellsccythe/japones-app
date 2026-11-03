import { test } from 'node:test';
import assert from 'node:assert/strict';
import { normalizeColors, DEFAULT_COLORS, CLASSES } from './colors.js';

test('empty uses defaults', () => assert.deepEqual(normalizeColors({}), DEFAULT_COLORS));

test('missing argument uses defaults', () => assert.deepEqual(normalizeColors(undefined), DEFAULT_COLORS));

test('invalid and unknown entries fall back', () => {
  const c = normalizeColors({ dark: { noun: 'red', verb: '#12', particle: '#AABBCC', foo: '#000000' } });
  assert.equal(c.dark.noun, DEFAULT_COLORS.dark.noun);
  assert.equal(c.dark.verb, DEFAULT_COLORS.dark.verb);
  assert.equal(c.dark.particle, '#AABBCC');
  assert.equal(c.dark.foo, undefined);
  assert.deepEqual(c.light, DEFAULT_COLORS.light);
});

test('defaults match the spec', () => {
  assert.deepEqual(CLASSES, ['noun', 'verb', 'particle', 'adjective', 'auxiliary']);
  assert.deepEqual(DEFAULT_COLORS.dark, { noun: '#7CC4E8', verb: '#F0937D', particle: '#9A9CA5', adjective: '#7FCBA4', auxiliary: '#C8A2E0' });
  assert.deepEqual(DEFAULT_COLORS.light, { noun: '#2F5FB3', verb: '#B2452E', particle: '#8A8780', adjective: '#2E7D5B', auxiliary: '#8E5BA8' });
});
