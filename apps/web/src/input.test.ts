import { describe, expect, it } from 'vitest';
import { parseNumber } from './input';

describe('parseNumber', () => {
  it('reads a finite number', () => {
    expect(parseNumber('12')).toBe(12);
    expect(parseNumber(' -3.5 ')).toBe(-3.5);
    expect(parseNumber('0')).toBe(0);
  });
  it('refuses empty, blank and non-finite input instead of turning it into 0', () => {
    for (const s of ['', '   ', 'abc', 'NaN', 'Infinity', '-Infinity', '1e999']) expect(parseNumber(s)).toBeNull();
  });
});
