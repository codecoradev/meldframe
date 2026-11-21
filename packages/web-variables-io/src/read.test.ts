import { afterEach, describe, expect, it } from 'vitest';
import { READ_CAP } from './io';
import { readFile } from './read';

// The tests run in node, which has no FileReader: a stand-in that answers as the browser's does, one task later.
type Outcome = { result: string | ArrayBuffer | null } | { error: true };
function stub(outcome: Outcome) {
  class FakeReader {
    result: string | ArrayBuffer | null = null;
    onload: (() => void) | null = null;
    onerror: (() => void) | null = null;
    readAsText() {
      setTimeout(() => {
        if ('error' in outcome) this.onerror?.();
        else {
          this.result = outcome.result;
          this.onload?.();
        }
      }, 0);
    }
  }
  (globalThis as unknown as { FileReader: unknown }).FileReader = FakeReader;
}
const file = (size: number) => ({ size, name: 'a.css' }) as unknown as File;

afterEach(() => {
  delete (globalThis as { FileReader?: unknown }).FileReader;
});

describe('reading a file into the field', () => {
  it('gives the text of a file the browser could read', async () => {
    stub({ result: ':root { --a: #fff; }' });
    expect(await readFile(file(20))).toEqual({ ok: true, text: ':root { --a: #fff; }' });
  });
  it('does not read a file over the cap that protects the browser, and says its size', async () => {
    stub({ result: 'never' });
    const r = await readFile(file(READ_CAP + 1));
    expect(r.ok).toBe(false);
    expect(r.ok === false && r.error).toMatch(/1\.0 MiB/);
  });
  it('says so when the browser cannot read the file, or gives something that is not text', async () => {
    stub({ error: true });
    expect(await readFile(file(5))).toEqual({ ok: false, error: 'This file could not be read.' });
    stub({ result: new ArrayBuffer(1) });
    expect(await readFile(file(5))).toEqual({ ok: false, error: 'This file could not be read as text.' });
  });
});
