import { describe, expect, it } from 'vitest';
import { FRESH, applied, edited, fileInto, fileRead, fileRefused, latest, saidFor, stranded, typed } from './flow';

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

describe('what the Import field holds', () => {
  const typedText = ':root { --a: #fff; }';
  it('starts empty and keeps what is typed, without a note about a file', () => {
    expect(FRESH).toEqual({ text: '', note: null });
    expect(typed(fileRead('a.css', '1 byte', 'x'), typedText)).toEqual({ text: typedText, note: null });
  });
  it('says a file was read, with its name and size, and puts its text in the field', () => {
    const v = fileRead('theme.css', '2.0 KiB', typedText);
    expect(v.text).toBe(typedText);
    expect(v.note).toEqual({ ok: true, text: 'Read theme.css (2.0 KiB). Check it, then import.' });
  });
  it('keeps the field as it was when a file is refused, and says why', () => {
    const v = fileRefused({ text: 'mine', note: null }, 'This file is 2.0 MiB.');
    expect(v).toEqual({ text: 'mine', note: { ok: false, text: 'This file is 2.0 MiB.' } });
  });
  it('starts again after an import that was applied: no text and no note about the file', () => {
    expect(applied()).toEqual(FRESH);
    expect(applied().note).toBeNull();
  });
});

describe('the latest of the things that finish later', () => {
  it('counts the last one started only', () => {
    const g = latest();
    const a = g.start();
    const b = g.start();
    expect(g.current(a)).toBe(false);
    expect(g.current(b)).toBe(true);
  });
  it('makes the ones under way stale when it is cancelled (a dialog that closed, a Clear)', () => {
    const g = latest();
    const a = g.start();
    g.cancel();
    expect(g.current(a)).toBe(false);
    expect(g.current(g.start())).toBe(true);
  });
});

describe('a file that is read while the dialog changes', () => {
  const size = (n: number) => `${n} bytes`;
  const later = () => {
    let resolve!: (r: { ok: true; text: string } | { ok: false; error: string }) => void;
    const p = new Promise<{ ok: true; text: string } | { ok: false; error: string }>((r) => (resolve = r));
    return { p, resolve };
  };

  it('fills the field when it is the latest and nothing happened meanwhile', async () => {
    const g = latest();
    const v = await fileInto(g, { name: 'a.css', size: 3 }, () => FRESH, async () => ({ ok: true, text: 'abc' }), size);
    expect(v).toEqual(fileRead('a.css', '3 bytes', 'abc'));
  });
  it('is ignored when the dialog closed before it was read', async () => {
    const g = latest();
    const slow = later();
    const pending = fileInto(g, { name: 'a.css', size: 3 }, () => FRESH, () => slow.p, size);
    g.cancel(); // the dialog closed, or Clear was pressed
    slow.resolve({ ok: true, text: 'abc' });
    expect(await pending).toBeNull();
  });
  it('is ignored when another file was chosen after it, whichever is read first', async () => {
    const g = latest();
    const first = later();
    const second = later();
    const a = fileInto(g, { name: 'a.css', size: 1 }, () => FRESH, () => first.p, size);
    const b = fileInto(g, { name: 'b.css', size: 1 }, () => FRESH, () => second.p, size);
    second.resolve({ ok: true, text: 'B' });
    first.resolve({ ok: true, text: 'A' });
    expect(await a).toBeNull();
    expect((await b)?.text).toBe('B');
  });
  it('is ignored when the person typed while it was being read, and what they typed stays', async () => {
    const g = latest();
    const slow = later();
    let view = FRESH;
    const pending = fileInto(g, { name: 'a.css', size: 3 }, () => view, () => slow.p, size);
    view = edited(g, view, ':root { --mine: 1; }');
    slow.resolve({ ok: true, text: 'the file' });
    expect(await pending).toBeNull();
    expect(view).toEqual({ text: ':root { --mine: 1; }', note: null });
  });
  it('a file chosen after typing is read as usual', async () => {
    const g = latest();
    const view = edited(g, FRESH, 'typed');
    expect(view.text).toBe('typed');
    expect(await fileInto(g, { name: 'a.css', size: 3 }, () => view, async () => ({ ok: true, text: 'abc' }), size)).toEqual(fileRead('a.css', '3 bytes', 'abc'));
  });
  it('a refusal that is late is ignored too, and one that is on time keeps the field', async () => {
    const g = latest();
    const now = () => ({ text: 'mine', note: null });
    expect(await fileInto(g, { name: 'a', size: 9 }, now, async () => ({ ok: false, error: 'big' }), size)).toEqual({ text: 'mine', note: { ok: false, text: 'big' } });
    const slow = later();
    const pending = fileInto(g, { name: 'a', size: 9 }, now, () => slow.p, size);
    g.cancel();
    slow.resolve({ ok: false, error: 'big' });
    expect(await pending).toBeNull();
  });
});

describe('where the focus is when an import empties the field', () => {
  const body = {};
  it('is stranded on nothing, on the page body, and on a control that became disabled', () => {
    expect(stranded(null, body)).toBe(true);
    expect(stranded(undefined, body)).toBe(true);
    expect(stranded(body, body)).toBe(true);
    expect(stranded({ disabled: true }, body)).toBe(true);
  });
  it('is not stranded on a control that works (the field, Close) or on an element that has no disabled state', () => {
    expect(stranded({ disabled: false }, body)).toBe(false);
    expect(stranded({}, body)).toBe(false);
  });
});
