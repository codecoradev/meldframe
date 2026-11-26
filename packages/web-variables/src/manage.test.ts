import { readFileSync } from 'node:fs';
import type { CollectionRow, Event, ManageLimits, NameDraft, NameError, Removal, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { MAX_COLLECTIONS, MAX_MODES, MAX_VARIABLES, NOTE_CLASS, addNote, collectionSummary, count, nameError, nameWarning, partial, reactTo, removalWords, renamedWords, typed } from './manage';

// The rules of the manage view (the checks of a name, what a removal detaches, the rename flow) are the app layer's and are
// tested there (crates/core/meldframe-variables-manage-tests); these are the words each code and number is given.
const limits: ManageLimits = { nameMax: 256, modeNameMax: 64, collections: 32, modes: 16, variables: 5000 };
const row = (over: Partial<VariableRow> = {}): VariableRow => ({ id: 'v1', name: 'color/bg', type: 'color', values: [], usedBy: 2, binds: [], ...over });
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c1', name: 'theme', modes: [{ id: 'm1', name: 'light' }], active: 'm1', matched: 2, variables: [row(), row({ id: 'v2', usedBy: 0 })], ...over });
const panel = (c: CollectionRow[], over: Partial<VariablesPanel> = {}): VariablesPanel => ({ query: '', total: 2, matched: 2, collections: c, ...over });
const removal = (over: Partial<Removal> = {}): Removal => ({ target: { kind: 'variable', id: 'v1' }, name: 'color/bg', detach: true, uses: 3, variables: 0, ...over });

describe('the words of a name that cannot be sent', () => {
  it('say what is wrong with it, for the kind of name', () => {
    const words = (kind: 'collection' | 'mode' | 'variable', e: NameError) => nameError(kind, e, limits);
    expect(words('variable', 'empty')).toBe('Give the variable a name.');
    expect(words('mode', 'empty')).toBe('Give the mode a name.');
    expect(words('collection', 'hidden')).toMatch(/invisible characters/);
    expect(words('variable', 'notPath')).toMatch(/lowercase letters, digits and single hyphens/);
    expect(words('variable', 'duplicate')).toBe('A variable in this collection already has this name.');
  });
  it('say the length of the kind, as the app layer gives it', () => {
    expect(nameError('variable', 'tooLong', limits)).toBe('Use 256 characters or fewer.');
    expect(nameError('mode', 'tooLong', limits)).toBe('Use 64 characters or fewer.');
    expect(nameError('mode', 'tooLong', { ...limits, modeNameMax: 10 })).toBe('Use 10 characters or fewer.');
  });
  it('word a name that exists already as a warning, naming where', () => {
    expect(nameWarning('collection', 'same')).toBe('A collection with this name already exists in this file. Both are kept, so tell them apart.');
    expect(nameWarning('mode', 'same')).toMatch(/in this collection/);
  });
});

describe('the line under a form that adds a name', () => {
  const draft = (text: string, check: Partial<NameDraft['check']> = {}): NameDraft => ({ text, check: { name: text.trim(), ...check } });
  it('says a refusal in the words of its code once something was typed, and nothing of it for an empty field', () => {
    expect(addNote('collection', draft('Colour', { error: 'notPath' }), limits)).toEqual({ tone: 'error', text: nameError('collection', 'notPath', limits) });
    expect(addNote('variable', draft('', { name: '', error: 'empty' }), limits)?.tone).toBe('hint');
    expect(addNote('variable', draft('   ', { name: '', error: 'empty' }), limits)?.tone).toBe('hint');
    expect(typed('  ')).toBe(false);
    expect(typed(' a ')).toBe(true);
  });
  it('says a name that exists already as a warning, naming where', () => {
    expect(addNote('mode', draft('dark', { warning: 'same' }), limits)).toEqual({ tone: 'warning', text: nameWarning('mode', 'same') });
  });
  it('gives a collection and a variable the rule of a name as a hint, and a mode nothing', () => {
    expect(addNote('collection', draft('ok'), limits)).toEqual({ tone: 'hint', text: nameError('collection', 'notPath', limits) });
    expect(addNote('variable', draft('ok'), limits)?.tone).toBe('hint');
    expect(addNote('mode', draft('ok'), limits)).toBeNull();
  });
  it('has a class for each tone, whole, so that Tailwind finds it', () => {
    expect(NOTE_CLASS).toEqual({ error: 'text-danger-text', warning: 'text-text-strong', hint: 'text-text-muted' });
  });
});

describe('the summary of a collection', () => {
  it('counts its variables, their uses and its modes', () => {
    expect(collectionSummary(col(), panel([col()]))).toEqual({ variables: 2, uses: 2, modes: 1, complete: true });
  });
  it('says the counts are partial when the list is cut or a search narrows it', () => {
    expect(collectionSummary(col({ matched: 3 }), panel([col({ matched: 3 })])).complete).toBe(false);
    expect(collectionSummary(col(), panel([col()], { query: 'bg' })).complete).toBe(false);
  });
});

describe('the words of the question before a removal', () => {
  it('removes a variable nothing uses plainly', () => {
    const w = removalWords(removal({ detach: false, uses: 0 }));
    expect(w.confirm).toBe('Remove');
    expect(w.text).toMatch(/Nothing uses it/);
    expect(w.done).toBe('Removed “color/bg”.');
  });
  it('offers Detach for a variable that is used, says how many uses and that undo is one step', () => {
    const w = removalWords(removal({ uses: 3 }));
    expect(w.confirm).toBe('Detach');
    expect(w.text).toContain('3 uses');
    expect(w.text).toMatch(/One undo puts it all back/);
    expect(w.done).toBe('Detached 3 uses and removed “color/bg”.');
    expect(removalWords(removal({ uses: 1 })).text).toContain('1 use ');
  });
  it('removes an empty collection plainly and detaches one that holds variables, even when nothing uses them', () => {
    const empty = removalWords(removal({ target: { kind: 'collection', id: 'c1' }, name: 'theme', detach: false, uses: 0, variables: 0 }));
    expect(empty.confirm).toBe('Remove');
    expect(empty.text).toMatch(/empty collection/);
    const held = removalWords(removal({ target: { kind: 'collection', id: 'c1' }, name: 'theme', detach: true, uses: 0, variables: 2 }));
    expect(held.confirm).toBe('Detach');
    expect(held.text).toMatch(/2 variables\? Nothing uses them/);
  });
  it('says that aliases are uses, and counts a single use in the singular', () => {
    const w = removalWords(removal({ target: { kind: 'collection', id: 'c1' }, name: 'theme', detach: true, uses: 1, variables: 2 }));
    expect(w.text).toMatch(/There is 1 use in the file/);
    expect(w.text).toMatch(/aliases among these variables count too/);
    expect(removalWords(removal({ target: { kind: 'collection', id: 'c1' }, name: 'theme', detach: true, uses: 4, variables: 2 })).text).toMatch(/There are 4 uses/);
  });
  it('removes a mode with the values it holds, and names a name that is nothing in words', () => {
    const w = removalWords(removal({ target: { kind: 'mode', collection: 'c1', id: 'm2' }, name: 'dark', detach: false, uses: 0 }));
    expect(w.confirm).toBe('Remove');
    expect(w.text).toMatch(/Its values go/);
    expect(removalWords(removal({ name: '', detach: false, uses: 0 })).done).toBe('Removed “(no name)”.');
  });
});

describe('reacting to the events of a dispatch', () => {
  const refused = (code: string): Event => ({ type: 'refused', code });
  it('words a refusal and keeps the thing that was added apart', () => {
    expect(reactTo([refused('E_INVALID_NAME')], null, limits).error).toMatch(/lowercase/);
    const r = reactTo([{ type: 'entityAdded', entity: 'variable', id: 'v9' }, { type: 'documentChanged', actor: 'a', optimistic: true }], 'Added.', limits);
    expect(r).toMatchObject({ error: null, added: { entity: 'variable', id: 'v9' }, status: 'Added.' });
  });
  it('words a rejection the server sent for a removal that lost a race', () => {
    expect(reactTo([{ type: 'opRejected', key: 'k', code: 'E_VARIABLE_IN_USE', node: null, retryable: false }], null, limits).error).toMatch(/Detach/);
  });
  it('words the limit of what was sent, from the limits of the app layer, not a limit of the file in general', () => {
    const hit = (sent: object) => reactTo([refused('E_TOO_MANY_ENTRIES')], null, limits, sent as never).error;
    expect(hit({ type: 'createCollection', name: 'x' })).toBe('A file holds at most 32 collections.');
    expect(hit({ type: 'addMode', collection: 'c', name: 'x' })).toBe('A collection holds at most 16 modes.');
    expect(hit({ type: 'addVariable', collection: 'c', name: 'x', kind: 'color' })).toBe('A file holds at most 5,000 variables.');
    expect(reactTo([refused('E_TOO_MANY_ENTRIES')], null, { ...limits, modes: 4 }, { type: 'addMode', collection: 'c', name: 'x' } as never).error).toBe('A collection holds at most 4 modes.');
    expect(reactTo([refused('E_TOO_LONG')], null, { ...limits, nameMax: 100 }, { type: 'createCollection', name: 'x' } as never).error).toBe('That name is too long. Use 100 characters or fewer.');
  });
  it('keeps the general words for a limit when what was sent is not known or is not an addition', () => {
    expect(reactTo([refused('E_TOO_MANY_ENTRIES')], null, limits).error).toMatch(/limit/);
    expect(reactTo([refused('E_TOO_MANY_ENTRIES')], null, limits, { type: 'removeMode', collection: 'c', mode: 'm' } as never).error).toMatch(/limit/);
  });
  it('turns the pending words into the status of a change that was made, and into nothing for a refused one', () => {
    const changed: Event = { type: 'documentChanged', actor: 'a', optimistic: true };
    expect(reactTo([changed], 'Removed.', limits).status).toBe('Removed.');
    expect(reactTo([changed, refused('E_VARIABLE_IN_USE')], 'Removed.', limits).status).toBeNull();
    expect(reactTo([], 'Removed.', limits).status).toBeNull();
  });
  it('says what a rename made, from the event, and names the entity that was renamed', () => {
    const r = reactTo([{ type: 'renamed', entity: 'mode', from: 'dark', to: 'night' }, { type: 'documentChanged', actor: 'a', optimistic: true }], null, limits);
    expect(r.renamed).toEqual({ entity: 'mode', from: 'dark', to: 'night' });
    expect(r.status).toBe('Renamed the mode “dark” to “night”.');
    expect(renamedWords({ entity: 'variable', from: 'a', to: 'b' })).toBe('Renamed “a” to “b”.');
    expect(renamedWords({ entity: 'collection', from: '', to: 'b' })).toBe('Renamed the collection “(no name)” to “b”.');
  });
});

describe('why the counts may be partial', () => {
  it('is a search that narrows the list, or a list that was cut, and nothing otherwise', () => {
    expect(partial(panel([col()]))).toBeNull();
    expect(partial(panel([col()], { query: ' x ' }))).toBe('search');
    expect(partial(panel([col({ matched: 300 })]))).toBe('cut');
  });
});

describe('count', () => {
  it('is singular for one', () => {
    expect([count(1, 'use', 'uses'), count(0, 'use', 'uses'), count(2, 'use', 'uses')]).toEqual(['1 use', '0 uses', '2 uses']);
  });
});

describe('the limits copied for the import view', () => {
  // The manage view reads `state.manage.limits`; the import view has no state, so it keeps copies, which this reads from the core (finding T3).
  const limitsRs = readFileSync(new URL('../../../crates/core/meldframe-check/src/limits.rs', import.meta.url), 'utf8');
  it('are the core limits', () => {
    expect(limitsRs).toMatch(new RegExp(`max_collections: ${MAX_COLLECTIONS},`));
    expect(limitsRs).toMatch(new RegExp(`max_modes: ${MAX_MODES},`));
    expect(limitsRs).toMatch(new RegExp(`max_variables: ${String(MAX_VARIABLES).replace(/(\d)(?=(\d{3})+$)/, '$1_')},`));
  });
});
