import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { CollectionRow, Event, VariableRow, VariablesPanel } from '@meldframe/protocol';
import {
  LABEL_MAX,
  NAME_MAX,
  addModeAction,
  addVariableAction,
  checkName,
  collectionSummary,
  label,
  createCollectionAction,
  partial,
  plain,
  reactTo,
  removalPlan,
  removeCollectionAction,
  removeModeAction,
  removeVariableAction,
  targetOf,
  whereAdded,
} from './manage';

// The core's rules are read from its source, so a rule changed there and not here fails these tests (as web-shell reads
// the frame presets and web-activity the refusal codes).
const root = new URL('../../../', import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), 'utf8');
const checkRs = read('crates/core/meldframe-variables/src/check.rs');
const limitsRs = read('crates/core/meldframe-check/src/limits.rs');
const limit = (field: string) => Number(new RegExp(`${field}: ([\\d_]+),`).exec(limitsRs)![1]!.replace(/_/g, ''));
const nameRule = checkRs.slice(checkRs.indexOf('fn valid_name'), checkRs.indexOf('fn collection_of'));

const row = (over: Partial<VariableRow> = {}): VariableRow => ({ id: 'v1', name: 'color/bg', type: 'color', values: [], usedBy: 0, binds: [], ...over });
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({
  id: 'c1',
  name: 'theme',
  modes: [{ id: 'm1', name: 'light' }, { id: 'm2', name: 'dark' }],
  active: 'm1',
  matched: 2,
  variables: [row({ usedBy: 2 }), row({ id: 'v2', name: 'space/4', type: 'number', usedBy: 0 })],
  ...over,
});
const panel = (collections: CollectionRow[], over: Partial<VariablesPanel> = {}): VariablesPanel => ({ query: '', total: 2, matched: 2, collections, ...over });

describe('the name rule of variables and collections', () => {
  it('is the core rule: pinned in its source and in its limit', () => {
    for (const piece of ['split(\'/\')', '!s.is_empty()', "starts_with('-')", "ends_with('-')", 'contains("--")', 'is_ascii_lowercase', 'is_ascii_digit', "b == b'-'"]) expect(nameRule, piece).toContain(piece);
    expect(NAME_MAX).toBe(limit('max_name'));
  });
  it('lets through lowercase ASCII slash paths', () => {
    for (const name of ['a', 'color/brand-500', 'radius/card', 'space/4', 'a-b/c-d/e']) expect(checkName('variable', name, []).error, name).toBeNull();
  });
  it('refuses what the core refuses, with the rule in words', () => {
    for (const name of ['Brand', 'a--b', 'a b', 'color/', '/a', 'a//b', '-a', 'a-', 'a_b', 'é', 'Color/Ink']) {
      expect(checkName('collection', name, []).error, name).toMatch(/lowercase/i);
    }
  });
  it('refuses the names the app-layer tests send as bad', () => {
    const tests = read('crates/core/meldframe-variables-manage-tests/tests/create.rs');
    const bad = /for name in \[([^\]]*)\]/.exec(tests.slice(tests.indexOf('fn a_bad_collection_name')))![1]!;
    const names = [...bad.matchAll(/"([^"]*)"/g)].map((m) => m[1]!);
    expect(names.length).toBeGreaterThan(2);
    for (const name of names) expect(checkName('collection', name, []).error, JSON.stringify(name)).not.toBeNull();
  });
  it('asks for a name, trims the spaces around it, and says how long it may be', () => {
    expect(checkName('variable', '', []).error).toMatch(/name/i);
    expect(checkName('variable', '   ', []).error).toMatch(/name/i);
    expect(checkName('variable', '  color/bg ', []).name).toBe('color/bg');
    expect(checkName('variable', 'a'.repeat(NAME_MAX), []).error).toBeNull();
    expect(checkName('variable', 'a'.repeat(NAME_MAX + 1), []).error).toContain(String(NAME_MAX));
  });
  it('refuses control, direction and invisible characters in every kind of name, before the rule of the kind', () => {
    for (const kind of ['collection', 'mode', 'variable'] as const) {
      // Direction and control characters, and what is drawn as nothing: Hangul fillers, the blank braille cell, other spaces than the ASCII one.
      for (const bad of ['dark\u202e', 'da\u200brk', 'da\u0007rk', 'a\u2066b', 'a\nb', 'dark\u3164', 'a\u115fb', 'a\u1160b', 'a\uffa0b', 'a\u2800b', 'a\u00a0b', 'a\u3000b', 'a\u2009b', 'a\u034fb', 'a\u17b4b', 'a\u17b5b']) {
        expect(checkName(kind, bad, []).error, `${kind} ${JSON.stringify(bad)}`).toMatch(/control, direction or invisible/i);
      }
    }
  });
});

describe('the name of a mode', () => {
  it('is free text of a limited length: capital letters and spaces are fine, empty is not', () => {
    expect(checkName('mode', 'Dark (high contrast)', []).error).toBeNull();
    expect(checkName('mode', '  ', []).error).toMatch(/name/i);
    expect(checkName('mode', 'x'.repeat(LABEL_MAX + 1), []).error).toContain(String(LABEL_MAX));
    expect(LABEL_MAX).toBeLessThanOrEqual(limit('max_name'));
  });
});

describe('a name that exists already', () => {
  it('is an error for a variable (the core refuses it) and compares the name as sent', () => {
    expect(checkName('variable', 'color/bg', ['color/bg']).error).toMatch(/already/i);
    expect(checkName('variable', ' color/bg ', ['color/bg']).error).toMatch(/already/i);
    expect(checkName('variable', 'color/bg', ['color/ink']).error).toBeNull();
  });
  it('is a warning for a collection or a mode (the core lets it through), ignoring case', () => {
    for (const [kind, name, existing] of [['collection', 'theme', ['theme']], ['mode', 'Dark', ['dark', 'light']]] as const) {
      const c = checkName(kind, name, [...existing]);
      expect(c.error, kind).toBeNull();
      expect(c.warning, kind).toMatch(/already/i);
    }
    expect(checkName('mode', 'Sepia', ['dark']).warning).toBeNull();
  });
  it('is no warning while the name is invalid or empty', () => {
    expect(checkName('collection', 'Theme', ['theme']).warning).toBeNull();
  });
});

describe('plain', () => {
  it('removes control, direction and zero-width characters and line separators from a name for display', () => {
    expect(plain('da\u202erk\u200b\n\u2028x')).toBe('darkx');
    expect(plain('color/bg')).toBe('color/bg');
  });
  it('removes what is drawn as nothing, so a name cannot pass for the badge beside it', () => {
    expect(plain('da\u3164rk\u115f\u1160\uffa0\u2800')).toBe('dark');
  });
  it('turns every other space into one plain space and drops the ones at the ends', () => {
    expect(plain('a\u00a0\u3000  b')).toBe('a b');
    expect(plain('  x\u2009')).toBe('x');
  });
});

describe('label', () => {
  it('is the plain name, or a word for a name that shows as nothing', () => {
    expect(label('da\u202erk')).toBe('dark');
    expect(label('\u3164\u2800')).toBe('(no name)');
    expect(label('')).toBe('(no name)');
  });
});

describe('the actions of the view', () => {
  it('are the six manage actions with the names the app layer takes', () => {
    expect(createCollectionAction('brand')).toEqual({ type: 'createCollection', name: 'brand' });
    expect(addModeAction('c1', 'dark', 'm1')).toEqual({ type: 'addMode', collection: 'c1', name: 'dark', copyFrom: 'm1' });
    expect(removeModeAction('c1', 'm2')).toEqual({ type: 'removeMode', collection: 'c1', mode: 'm2' });
    expect(addVariableAction('c1', 'color/ink', 'number')).toEqual({ type: 'addVariable', collection: 'c1', name: 'color/ink', kind: 'number' });
    expect(removeVariableAction('v1', true)).toEqual({ type: 'removeVariable', id: 'v1', detach: true });
    expect(removeCollectionAction('c1', false)).toEqual({ type: 'removeCollection', id: 'c1', detach: false });
  });
  it('leave copyFrom out when there is nothing to copy (the default mode is then copied)', () => {
    expect('copyFrom' in addModeAction('c1', 'dark', null)).toBe(false);
    expect('copyFrom' in addModeAction('c1', 'dark', '')).toBe(false);
  });
  it('send the trimmed name', () => {
    expect(createCollectionAction('  brand ')).toEqual({ type: 'createCollection', name: 'brand' });
  });
});

describe('the summary of a collection', () => {
  it('counts its variables, their uses and its modes', () => {
    expect(collectionSummary(col(), panel([col()]))).toEqual({ variables: 2, uses: 2, modes: 2, complete: true });
  });
  it('says the counts are partial when the list is cut or a search narrows it', () => {
    expect(collectionSummary(col({ matched: 300 }), panel([col({ matched: 300 })])).complete).toBe(false);
    expect(collectionSummary(col(), panel([col()], { query: 'bg' })).complete).toBe(false);
  });
});

describe('the confirmation of a removal', () => {
  it('removes a variable nothing uses plainly: no detach', () => {
    const p = removalPlan({ kind: 'variable', row: row({ usedBy: 0 }) });
    expect(p.action).toEqual({ type: 'removeVariable', id: 'v1', detach: false });
    expect(p.confirm).toBe('Remove');
    expect(p.text).toMatch(/nothing uses/i);
  });
  it('offers Detach for a variable that is used, says how many uses and that undo is one step', () => {
    const p = removalPlan({ kind: 'variable', row: row({ usedBy: 3 }) });
    expect(p.action).toEqual({ type: 'removeVariable', id: 'v1', detach: true });
    expect(p.confirm).toBe('Detach');
    expect(p.text).toMatch(/3 uses/);
    expect(p.text).toMatch(/value/i);
    expect(p.text).toMatch(/undo/i);
    expect(removalPlan({ kind: 'variable', row: row({ usedBy: 1 }) }).text).toMatch(/1 use\b/);
  });
  it('removes an empty collection plainly and detaches one that holds variables, even when nothing uses them', () => {
    const empty = removalPlan({ kind: 'collection', col: col({ matched: 0, variables: [] }), summary: { variables: 0, uses: 0, modes: 2, complete: true } });
    expect(empty.action).toEqual({ type: 'removeCollection', id: 'c1', detach: false });
    expect(empty.confirm).toBe('Remove');
    const unused = removalPlan({ kind: 'collection', col: col(), summary: { variables: 2, uses: 0, modes: 2, complete: true } });
    expect(unused.action).toEqual({ type: 'removeCollection', id: 'c1', detach: true });
    expect(unused.confirm).toBe('Detach');
    expect(unused.text).toMatch(/2 variables/);
  });
  it('does not say nothing uses a cut list of variables when no use was seen on it', () => {
    const p = removalPlan({ kind: 'collection', col: col(), summary: { variables: 300, uses: 0, modes: 2, complete: false } });
    expect(p.text).not.toMatch(/nothing uses them/i);
    expect(p.text).toMatch(/listed/i);
    expect(removalPlan({ kind: 'collection', col: col(), summary: { variables: 2, uses: 0, modes: 2, complete: true } }).text).toMatch(/nothing uses them/i);
  });
  it('says that aliases are uses, so a count that includes the aliases among the variables that go is not read as nodes only', () => {
    expect(removalPlan({ kind: 'variable', row: row({ usedBy: 3 }) }).text).toMatch(/node fields and aliases/);
    expect(removalPlan({ kind: 'collection', col: col(), summary: { variables: 2, uses: 4, modes: 2, complete: true } }).text).toMatch(/node fields and aliases/);
  });
  it('says "at least" when the uses were counted on a cut list', () => {
    const p = removalPlan({ kind: 'collection', col: col(), summary: { variables: 300, uses: 7, modes: 2, complete: false } });
    expect(p.text).toMatch(/at least 7 uses/);
  });
  it('removes a mode with the values it holds, and names it as plain text', () => {
    const p = removalPlan({ kind: 'mode', collection: 'c1', mode: { id: 'm2', name: 'da‮rk' } });
    expect(p.action).toEqual({ type: 'removeMode', collection: 'c1', mode: 'm2' });
    expect(p.text).toContain('“dark”');
    expect(p.text).toMatch(/other modes keep/i);
    expect(p.confirm).toBe('Remove');
  });
  it('names what goes without direction characters', () => {
    expect(removalPlan({ kind: 'variable', row: row({ name: 'a‮b' }) }).text).toContain('“ab”');
  });
});

describe('reacting to the events of a dispatch', () => {
  const refused = (code: string) => ({ type: 'refused', code }) as Event;
  const added = (entity: 'collection' | 'mode' | 'variable', id: string) => ({ type: 'entityAdded', entity, id }) as Event;
  it('words a refusal and keeps the thing that was added apart', () => {
    expect(reactTo([refused('E_VARIABLE_IN_USE')], null)).toMatchObject({ error: expect.stringMatching(/still uses/i), added: null });
    expect(reactTo([{ type: 'documentChanged', actor: 'a', optimistic: true } as Event, added('variable', 'v9')], 'Added.')).toEqual({ error: null, added: { entity: 'variable', id: 'v9' }, status: 'Added.' });
  });
  it('words a rejection the server sent for a removal that lost a race', () => {
    expect(reactTo([{ type: 'opRejected', key: 'k', code: 'E_VARIABLE_IN_USE', node: null, retryable: false } as Event], 'Removed.').error).toMatch(/still uses|started using/i);
  });
  it('words the limit of what was sent, not a limit of the file in general', () => {
    const limits = read('crates/core/meldframe-check/src/limits.rs');
    const n = (f: string) => Number(new RegExp(`${f}: ([\\d_]+),`).exec(limits)![1]!.replace(/_/g, ''));
    const many = [refused('E_TOO_MANY_ENTRIES')];
    expect(reactTo(many, null, createCollectionAction('a')).error).toContain(String(n('max_collections')));
    expect(reactTo(many, null, addModeAction('c1', 'a', null)).error).toContain(String(n('max_modes')));
    expect(reactTo(many, null, addVariableAction('c1', 'a', 'color')).error).toContain(n('max_variables').toLocaleString('en-US'));
    expect(reactTo([refused('E_TOO_LONG')], null, addVariableAction('c1', 'a', 'color')).error).toMatch(new RegExp(`name is too long.*${NAME_MAX}`));
    expect(reactTo([refused('E_DUPLICATE_ID')], null, createCollectionAction('a')).error).toMatch(/nothing was added/i);
  });
  it('keeps the general words for a limit when what was sent is not known or is not an addition', () => {
    for (const sent of [null, removeVariableAction('v1', true)]) {
      for (const code of ['E_TOO_LONG', 'E_TOO_MANY_ENTRIES', 'E_DUPLICATE_ID']) {
        expect(reactTo([refused(code)], null, sent).error, code).not.toMatch(/variable|collection|mode|name/i);
      }
    }
  });
  it('turns the pending words into the status of a change that was made, and into nothing for a refused one', () => {
    expect(reactTo([{ type: 'documentChanged', actor: 'a', optimistic: true } as Event], 'Removed.').status).toBe('Removed.');
    expect(reactTo([refused('outbox_full')], 'Removed.').status).toBeNull();
    expect(reactTo([{ type: 'toolChanged', tool: 'select' } as Event], 'Removed.')).toEqual({ error: null, added: null, status: null });
  });
});

describe('where an added thing is', () => {
  const p = panel([col(), col({ id: 'c2', name: 'brand', modes: [{ id: 'm9', name: 'default' }], variables: [row({ id: 'v7', name: 'color/x' })] })]);
  it('finds the collection of a variable, of a mode, and the collection itself', () => {
    expect(whereAdded(p, { entity: 'variable', id: 'v7' })).toBe('c2');
    expect(whereAdded(p, { entity: 'mode', id: 'm9' })).toBe('c2');
    expect(whereAdded(p, { entity: 'mode', id: 'm2' })).toBe('c1');
    expect(whereAdded(p, { entity: 'collection', id: 'c2' })).toBe('c2');
  });
  it('is nothing for what is not listed', () => {
    expect(whereAdded(p, { entity: 'variable', id: 'gone' })).toBeNull();
  });
});

describe('why the counts may be partial', () => {
  it('is a search that narrows the list, or a list that was cut, and nothing otherwise', () => {
    expect(partial(panel([col()]))).toBeNull();
    expect(partial(panel([col()], { query: ' bg ' }))).toBe('search');
    expect(partial(panel([col({ matched: 300 })]))).toBe('cut');
    expect(partial(panel([col()], { query: 'x' }))).toBe('search');
  });
});

describe('what a removal is about now', () => {
  const p = panel([col(), col({ id: 'c2', name: 'brand', variables: [row({ id: 'v7', name: 'color/x' })], matched: 1 })]);
  it('is looked up again in the panel by id', () => {
    expect(targetOf(p, { kind: 'variable', id: 'v7' })).toMatchObject({ kind: 'variable', row: { id: 'v7' } });
    expect(targetOf(p, { kind: 'collection', id: 'c2' })).toMatchObject({ kind: 'collection', col: { id: 'c2' }, summary: { variables: 1 } });
    expect(targetOf(p, { kind: 'mode', collection: 'c1', id: 'm2' })).toMatchObject({ kind: 'mode', collection: 'c1', mode: { name: 'dark' } });
  });
  it('is nothing for what is gone (the question then closes)', () => {
    expect(targetOf(p, { kind: 'variable', id: 'gone' })).toBeNull();
    expect(targetOf(p, { kind: 'collection', id: 'gone' })).toBeNull();
    expect(targetOf(p, { kind: 'mode', collection: 'c1', id: 'gone' })).toBeNull();
    expect(targetOf(p, { kind: 'mode', collection: 'gone', id: 'm2' })).toBeNull();
  });
});
