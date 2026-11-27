// F-273 (ADR 0837 U1): the words of a change's reason (src/renderer/project/activity.ts, whyOf): given or "not given",
// cut after ADR 0188 J4's 500 characters (Unicode code points) with an ellipsis.
import { expect, test } from '@playwright/test';
import { MAX_REASON, whyOf } from '../../src/renderer/project/activity.js';

test('U1: a reason of up to 500 code points is shown whole, a longer one cut, a missing one "not given"', () => {
  expect(MAX_REASON).toBe(500);
  const a500 = 'a'.repeat(500);
  expect(whyOf(a500)).toEqual({ text: a500, given: true, cut: false });
  const cut = whyOf('a'.repeat(501));
  expect(cut).toEqual({ text: `${a500}…`, given: true, cut: true });
  // one astral character is one code point (two UTF-16 units): 500 points, not cut
  const astral = `${'a'.repeat(499)}\u{1F600}`;
  expect(astral.length).toBe(501);
  expect(whyOf(astral)).toEqual({ text: astral, given: true, cut: false });
  // a cut never splits a surrogate pair
  const long = '\u{1F600}'.repeat(501);
  expect(whyOf(long).text).toBe(`${'\u{1F600}'.repeat(500)}…`);
  for (const absent of ['', '  ', '\n', null, undefined, 42, { why: 'x' }]) {
    expect(whyOf(absent)).toEqual({ text: 'not given', given: false, cut: false });
  }
  expect(whyOf('<b>bold</b> & "q"').text).toBe('<b>bold</b> & "q"');
});
