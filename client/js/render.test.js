import { test } from 'node:test';
import assert from 'node:assert/strict';
import { tokensHtml, copyText, vocabHtml, legendHtml } from './render.js';

test('tokensHtml colors by class and adds ruby', () => {
  const html = tokensHtml([
    { surface: '見学', class: 'noun', segments: [{ text: '見学', reading: 'けんがく' }] },
    { surface: 'に', class: 'particle', segments: [{ text: 'に', reading: null }] },
  ]);
  assert.match(html, /class="w w-noun"><ruby>見学<rt>けんがく<\/rt><\/ruby>/);
  assert.match(html, /class="w w-particle">に</);
});

test('tokensHtml escapes', () =>
  assert.doesNotMatch(tokensHtml([{ surface: '<b>', class: 'other', segments: [{ text: '<b>', reading: null }] }]), /<b>/));

test('copyText has no brackets', () => {
  const t = copyText('日本には見学する。', 'No Japão…', [
    { base: '日本', reading: 'にほん', class: 'noun', gloss: 'Japan' },
    { base: 'に', reading: null, class: 'particle', gloss: '' },
  ]);
  assert.equal(t, '日本には見学する。\nNo Japão…\n\n日本 (にほん) = Japan\nに = —');
  assert.doesNotMatch(t, /[\[\]]/);
});

test('copyText strips brackets coming from the data', () => {
  const t = copyText('a', 'b', [{ base: 'x', reading: null, class: 'noun', gloss: '[colloquial] thing' }]);
  assert.doesNotMatch(t, /[\[\]]/);
});

test('vocabHtml shows a dash for an empty gloss and escapes', () => {
  const html = vocabHtml([
    { base: '見学', reading: 'けんがく', class: 'noun', gloss: 'field trip' },
    { base: '<i>', reading: null, class: 'other', gloss: '' },
  ]);
  assert.match(html, /w-noun/);
  assert.match(html, /けんがく/);
  assert.match(html, /—/);
  assert.doesNotMatch(html, /<i>/);
});

test('vocabHtml of nothing is an empty state', () => assert.match(vocabHtml([]), /class="empty"/));

test('legendHtml has one clickable dot per class', () => {
  const html = legendHtml();
  for (const c of ['noun', 'verb', 'particle', 'adjective', 'auxiliary']) assert.match(html, new RegExp(`data-class="${c}"`));
  assert.match(html, /adj\.\/adv\./);
});

test('tokensHtml without ruby keeps the colors and drops the readings', () => {
  const html = tokensHtml([{ surface: '見学', class: 'noun', segments: [{ text: '見学', reading: 'けんがく' }] }], { ruby: false });
  assert.equal(html, '<span class="w w-noun">見学</span>');
});
