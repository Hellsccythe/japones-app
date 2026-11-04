import { test } from 'node:test';
import assert from 'node:assert/strict';
import { normalizeColors, DEFAULT_COLORS, CLASSES, suggestedColors, withColor, withoutColor } from './colors.js';

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

test('8 suggestions: the 5 defaults of the theme plus 3 extras', () => {
  assert.deepEqual(suggestedColors('dark'), ['#7CC4E8', '#F0937D', '#9A9CA5', '#7FCBA4', '#C8A2E0', '#E0B050', '#F08080', '#A0A0FF']);
  assert.deepEqual(suggestedColors('light'), ['#2F5FB3', '#B2452E', '#8A8780', '#2E7D5B', '#8E5BA8', '#A86F0C', '#C0392B', '#5B4BC4']);
});

test('withColor changes one theme only and keeps the saved object intact', () => {
  const saved = { light: {}, dark: { noun: '#111111' } };
  const next = withColor(saved, 'dark', 'verb', '#ABCDEF');
  assert.equal(next.dark.verb, '#ABCDEF');
  assert.equal(next.dark.noun, '#111111');
  assert.deepEqual(next.light, {});
  assert.equal(saved.dark.verb, undefined);
});

test('withColor ignores invalid colors and classes', () => {
  assert.deepEqual(withColor({}, 'dark', 'verb', 'red'), { light: {}, dark: {} });
  assert.deepEqual(withColor({}, 'dark', 'foo', '#ABCDEF'), { light: {}, dark: {} });
});

test('withoutColor restores the default of one class', () => {
  const next = withoutColor({ light: { verb: '#000000' }, dark: { verb: '#ABCDEF' } }, 'dark', 'verb');
  assert.equal(next.dark.verb, undefined);
  assert.equal(next.light.verb, '#000000');
  assert.equal(normalizeColors(next).dark.verb, DEFAULT_COLORS.dark.verb);
});
