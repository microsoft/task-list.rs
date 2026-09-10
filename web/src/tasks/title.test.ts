import { describe, expect, it } from 'vitest';

import { MAX_TITLE_LENGTH, validateTitle } from './title';

describe('validateTitle', () => {
  it('rejects a blank / whitespace-only title', () => {
    expect(validateTitle('')).toEqual({ ok: false, error: 'Title is required.' });
    expect(validateTitle('   ')).toMatchObject({ ok: false });
  });

  it('rejects a title longer than the max length', () => {
    const result = validateTitle('a'.repeat(MAX_TITLE_LENGTH + 1));
    expect(result.ok).toBe(false);
  });

  it('accepts a title of exactly the max length', () => {
    expect(validateTitle('a'.repeat(MAX_TITLE_LENGTH))).toEqual({
      ok: true,
      value: 'a'.repeat(MAX_TITLE_LENGTH),
    });
  });

  it('trims surrounding whitespace and returns the trimmed value', () => {
    expect(validateTitle('  Buy milk  ')).toEqual({ ok: true, value: 'Buy milk' });
  });

  it('measures length in code points, not UTF-16 units', () => {
    // Each astral emoji is one code point but two UTF-16 units; 200 of them is valid.
    expect(validateTitle('😀'.repeat(MAX_TITLE_LENGTH)).ok).toBe(true);
    expect(validateTitle('😀'.repeat(MAX_TITLE_LENGTH + 1)).ok).toBe(false);
  });
});
