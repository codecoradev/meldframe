import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { STORAGE_KEY } from '@meldframe/web-tokens/theme';

const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
const script = /<script>([\s\S]*?)<\/script>/.exec(html)?.[1] ?? '';

/** Runs the inline script of index.html against a fake page. */
function run(localStorage: { getItem(k: string): string | null }) {
  const documentElement = { dataset: {} as Record<string, string> };
  new Function('localStorage', 'document', script)(localStorage, { documentElement });
  return documentElement.dataset.theme;
}

describe('the inline script of index.html (applies the stored theme before first paint)', () => {
  it('reads the same key as the app', () => {
    expect(script).toContain(`'${STORAGE_KEY}'`);
  });
  it('applies a stored theme', () => {
    expect(run({ getItem: () => 'light' })).toBe('light');
    expect(run({ getItem: () => 'dark' })).toBe('dark');
  });
  it('ignores an unknown or missing value and a storage that throws', () => {
    expect(run({ getItem: () => 'purple' })).toBeUndefined();
    expect(run({ getItem: () => null })).toBeUndefined();
    expect(run({ getItem: () => { throw new Error('blocked'); } })).toBeUndefined();
  });
  it('is the only script outside the module entry, and uses no prefers-color-scheme', () => {
    expect(html.match(/<script/g)?.length).toBe(2);
    expect(html).not.toContain('prefers-color-scheme');
  });
});
