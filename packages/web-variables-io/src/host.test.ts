import { afterEach, describe, expect, it, vi } from 'vitest';
import { saidFor } from './flow';
import { copyExport } from './host';

afterEach(() => vi.unstubAllGlobals());

describe('what Copy says in the Export variables dialog', () => {
  it('says Copied. when the clipboard took the text, and names the text it was about', async () => {
    const written: string[] = [];
    vi.stubGlobal('navigator', { clipboard: { writeText: async (t: string) => void written.push(t) } });
    const said = await copyExport(':root {}\n');
    expect(written).toEqual([':root {}\n']);
    expect(said).toEqual({ ok: true, text: 'Copied.', css: ':root {}\n' });
  });
  it('says so, as a failure, when the browser refuses (no permission, an insecure page)', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: async () => Promise.reject(new Error('denied')) } });
    expect(await copyExport('a')).toEqual({ ok: false, text: 'Your browser did not allow copying.', css: 'a' });
  });
  it('says so when there is no clipboard at all', async () => {
    vi.stubGlobal('navigator', {});
    expect((await copyExport('a')).ok).toBe(false);
  });
  it('goes stale with the text it was about: another actor adding a variable under the open dialog', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: async () => {} } });
    const said = await copyExport('one');
    expect(saidFor(said, 'one')).toEqual({ ok: true, text: 'Copied.' });
    expect(saidFor(said, 'one\ntwo')).toBeNull();
  });
});
