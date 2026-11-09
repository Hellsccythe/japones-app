import { test } from 'node:test';
import assert from 'node:assert/strict';
import { insetVars } from './insets.js';

test('the Android insets become CSS variables in px', () =>
  assert.deepEqual(insetVars('{"top":48,"right":0,"bottom":24,"left":12.5}'), {
    '--sa-top': '48px',
    '--sa-right': '0px',
    '--sa-bottom': '24px',
    '--sa-left': '12.5px',
  }));

test('missing or broken insets set nothing', () => {
  assert.deepEqual(insetVars('{"top":30}'), { '--sa-top': '30px' });
  assert.deepEqual(insetVars('{"top":-4,"bottom":"x"}'), {});
  assert.deepEqual(insetVars('not json'), {});
  assert.deepEqual(insetVars(undefined), {});
});
