import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { allTokens } from './tokens';

// U4: the tokens module must equal the design file (`DESIGN.md` at the repository root). The design
// file lists every token as `key = value` between two markers; `tokens.test.ts` then holds
// `tokens.css` equal to `tokens.ts`, so all three agree.

const design = readFileSync(new URL('../../../DESIGN.md', import.meta.url), 'utf8');

function designTokens(): [string, string][] {
  const start = design.indexOf('<!-- tokens:start -->');
  const end = design.indexOf('<!-- tokens:end -->');
  expect(start).toBeGreaterThanOrEqual(0);
  expect(end).toBeGreaterThan(start);
  return design
    .slice(start, end)
    .split('\n')
    .filter((line) => line.includes(' = '))
    .map((line) => {
      const [key, ...value] = line.split(' = ');
      return [key!.trim(), value.join(' = ').trim()];
    });
}

/** Hex colours compare without case; every other value compares as written. */
const norm = (v: string) => (v.startsWith('#') ? v.toLowerCase() : v);

describe('tokens.ts equals DESIGN.md', () => {
  it('has no duplicate key in the design file', () => {
    const keys = designTokens().map(([k]) => k);
    expect(new Set(keys).size).toBe(keys.length);
    expect(keys.length).toBeGreaterThan(100);
  });
  it('lists exactly the same keys', () => {
    const want = designTokens().map(([k]) => k).sort();
    const got = allTokens().map(([k]) => k).sort();
    expect(got).toEqual(want);
  });
  it('has the same value for every key', () => {
    const want = new Map(designTokens().map(([k, v]) => [k, norm(v)]));
    const wrong = allTokens()
      .filter(([k, v]) => want.get(k) !== norm(v))
      .map(([k, v]) => `${k}: tokens.ts has ${v}, DESIGN.md has ${want.get(k)}`);
    expect(wrong).toEqual([]);
  });
});
