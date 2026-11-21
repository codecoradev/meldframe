import { describe, expect, it } from 'vitest';
import { saidFor } from './flow';

describe('what the last Copy said', () => {
  it('is shown while the text on view is the text that was copied', () => {
    expect(saidFor({ ok: true, text: 'Copied.', css: ':root {}' }, ':root {}')).toEqual({ ok: true, text: 'Copied.' });
  });
  it('goes when the text changes under the open dialog', () => {
    expect(saidFor({ ok: true, text: 'Copied.', css: ':root {}' }, ':root {\n  --a: 1;\n}')).toBeNull();
  });
  it('is nothing before any Copy', () => {
    expect(saidFor(null, '')).toBeNull();
  });
  it('keeps a refusal of the browser until the text changes too', () => {
    expect(saidFor({ ok: false, text: 'Your browser did not allow copying.', css: 'a' }, 'a')?.ok).toBe(false);
    expect(saidFor({ ok: false, text: 'x', css: 'a' }, 'b')).toBeNull();
  });
});
