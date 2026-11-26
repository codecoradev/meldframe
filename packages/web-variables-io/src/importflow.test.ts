import { describe, expect, it } from 'vitest';
import type { Action, VariablesImportResult } from '@meldframe/protocol';
import { createImportFlow } from './importflow';
import type { Read } from './read';

const result = (status: VariablesImportResult['status'] = 'refused'): VariablesImportResult => ({ status, bytes: 1, limit: 262144, maxVariables: 50, report: null });
const file = (name = 'theme.css', size = 12) => ({ name, size });
/** A reader that finishes when the test says so, as a FileReader does. */
function reads() {
  const waiting: ((r: Read) => void)[] = [];
  return { read: () => new Promise<Read>((resolve) => waiting.push(resolve)), finish: (r: Read, i = 0) => waiting[i]!(r) };
}
function setup(current: VariablesImportResult | null = null) {
  const sent: Action[] = [];
  const r = reads();
  let held = current;
  const flow = createImportFlow({ dispatch: (a) => sent.push(a), result: () => held, read: r.read });
  const seen: string[] = [];
  flow.subscribe(() => seen.push(flow.view().text));
  return { flow, sent, r, seen, hold: (x: VariablesImportResult | null) => (held = x) };
}
const CLOSE = { type: 'closeVariablesImport' };
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the field of the Import variables dialog, as a flow', () => {
  it('starts empty, keeps what is typed, and tells whoever listens', () => {
    const { flow, seen } = setup();
    expect(flow.view()).toEqual({ text: '', note: null });
    flow.edit(':root{}');
    expect(flow.view().text).toBe(':root{}');
    expect(seen).toEqual([':root{}']);
  });
  it('puts the result of an earlier text away when the text changes, and sends nothing when there is none', () => {
    const none = setup(null);
    none.flow.edit('a');
    expect(none.sent).toEqual([]);
    const shown = setup(result());
    shown.flow.edit('a');
    expect(shown.sent).toEqual([CLOSE]);
  });
  it('Clear empties the field and the note, puts the result away, and ends a file that is being read', async () => {
    const { flow, sent, r } = setup(result());
    flow.edit('typed');
    sent.length = 0;
    const done = flow.choose(file());
    flow.clear();
    expect(flow.view()).toEqual({ text: '', note: null });
    expect(sent).toEqual([CLOSE]);
    r.finish({ ok: true, text: 'late' });
    await done;
    expect(flow.view().text).toBe('');
  });
  it('a file that is read fills the field and says so, and puts the result of an earlier text away', async () => {
    const { flow, sent, r } = setup(result());
    const done = flow.choose(file('theme.css', 12));
    r.finish({ ok: true, text: ':root{}' });
    await done;
    expect(flow.view()).toEqual({ text: ':root{}', note: { ok: true, text: 'Read theme.css (12 bytes). Check it, then import.' } });
    expect(sent).toEqual([CLOSE]);
  });
  it('a file that is not read leaves the text of the field as it was, with the reason (the result goes, as for any file, as the Svelte dialog does)', async () => {
    const { flow, sent, r } = setup(result());
    flow.edit('mine');
    sent.length = 0;
    const done = flow.choose(file());
    r.finish({ ok: false, error: 'This file could not be read.' });
    await done;
    expect(flow.view()).toEqual({ text: 'mine', note: { ok: false, text: 'This file could not be read.' } });
    expect(sent).toEqual([CLOSE]);
  });
  it('what is typed while a file is being read stays: the file never replaces it', async () => {
    const { flow, r } = setup();
    const done = flow.choose(file());
    flow.edit('typed meanwhile');
    r.finish({ ok: true, text: 'from the file' });
    await done;
    expect(flow.view().text).toBe('typed meanwhile');
  });
  it('closing the dialog tells the app layer, keeps the text, and a file still being read never fills the field the person left', async () => {
    const { flow, sent, r } = setup();
    flow.edit('keep me');
    const done = flow.choose(file());
    flow.close();
    expect(sent).toEqual([CLOSE]);
    expect(flow.view().text).toBe('keep me');
    r.finish({ ok: true, text: 'late' });
    await done;
    expect(flow.view().text).toBe('keep me');
  });
  it('of two files chosen one after the other only the last counts, whichever is read first', async () => {
    const { flow, r } = setup();
    const first = flow.choose(file('a.css'));
    const second = flow.choose(file('b.css'));
    r.finish({ ok: true, text: 'second' }, 1);
    await second;
    r.finish({ ok: true, text: 'first' }, 0);
    await first;
    expect(flow.view().text).toBe('second');
    expect(flow.view().note?.text).toMatch(/^Read b\.css/);
  });
  it('an import that was applied starts the field again, with no note about a file', async () => {
    const { flow, r } = setup();
    const done = flow.choose(file());
    r.finish({ ok: true, text: ':root{}' });
    await done;
    flow.imported();
    expect(flow.view()).toEqual({ text: '', note: null });
  });
  it('reads the result when it acts, not when it was made (the app layer answers after the click)', async () => {
    const { flow, sent, r, hold } = setup(null);
    const done = flow.choose(file());
    hold(result());
    r.finish({ ok: true, text: 'x' });
    await done;
    expect(sent).toEqual([CLOSE]);
  });
  it('stops telling a listener that left', async () => {
    const { flow, seen } = setup();
    const off = flow.subscribe(() => seen.push('second'));
    flow.edit('a');
    off();
    flow.edit('b');
    await tick();
    expect(seen).toEqual(['a', 'second', 'b']);
  });
});
