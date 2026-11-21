import { describe, expect, it } from 'vitest';
import type { CollectionRow, Event, VariableRow, VariablesPanel } from '@meldframe/protocol';
import { LABEL_MAX, NAME_MAX, reactTo } from './manage';
import { NO_RENAME, RENAME_HINT, answerRename, checkRename, closeRename, goneWords, movedWords, openRename, renameAction, renameCollectionAction, renameContext, renameHint, renameModeAction, renameVariableAction, renamedWords, sendRename } from './rename';

const row = (over: Partial<VariableRow> = {}): VariableRow => ({ id: 'v1', name: 'color/bg', type: 'color', values: [], usedBy: 0, binds: [], ...over });
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({
  id: 'c1',
  name: 'theme',
  modes: [{ id: 'm1', name: 'light' }, { id: 'm2', name: 'dark' }],
  active: 'm1',
  matched: 2,
  variables: [row(), row({ id: 'v2', name: 'color/ink' })],
  ...over,
});
const other = col({ id: 'c2', name: 'brand', modes: [{ id: 'm9', name: 'default' }], matched: 1, variables: [row({ id: 'v9', name: 'color/bg' })] });
const panel = (collections: CollectionRow[] = [col(), other]): VariablesPanel => ({ query: '', total: 3, matched: 3, collections });

describe('the actions of a rename', () => {
  it('are one editVariables action with one rename command, the name trimmed', () => {
    expect(renameCollectionAction('c1', ' theme-2 ')).toEqual({ type: 'editVariables', commands: [{ type: 'renameCollection', id: 'c1', name: 'theme-2' }] });
    expect(renameModeAction('c1', 'm2', ' Night ')).toEqual({ type: 'editVariables', commands: [{ type: 'renameMode', collection: 'c1', mode: 'm2', name: 'Night' }] });
    expect(renameVariableAction('v1', 'color/surface')).toEqual({ type: 'editVariables', commands: [{ type: 'renameVariable', id: 'v1', name: 'color/surface' }] });
  });
  it('are built for the thing that was asked about, by its kind', () => {
    expect(renameAction({ kind: 'collection', id: 'c1' }, 'x')).toEqual(renameCollectionAction('c1', 'x'));
    expect(renameAction({ kind: 'mode', collection: 'c1', id: 'm2' }, 'x')).toEqual(renameModeAction('c1', 'm2', 'x'));
    expect(renameAction({ kind: 'variable', id: 'v1' }, 'x')).toEqual(renameVariableAction('v1', 'x'));
  });
});

describe('what a rename is checked against', () => {
  it('is the current name and the names of the others of its kind, never its own', () => {
    expect(renameContext(panel(), { kind: 'variable', id: 'v1' })).toEqual({ current: 'color/bg', others: ['color/ink'] });
    expect(renameContext(panel(), { kind: 'collection', id: 'c1' })).toEqual({ current: 'theme', others: ['brand'] });
    expect(renameContext(panel(), { kind: 'mode', collection: 'c1', id: 'm2' })).toEqual({ current: 'dark', others: ['light'] });
  });
  it('compares a variable with its own collection only: another collection may use the same name', () => {
    expect(renameContext(panel(), { kind: 'variable', id: 'v9' })).toEqual({ current: 'color/bg', others: [] });
  });
  it('is null for a thing another actor removed, so the field can close', () => {
    expect(renameContext(panel(), { kind: 'variable', id: 'nope' })).toBeNull();
    expect(renameContext(panel(), { kind: 'collection', id: 'nope' })).toBeNull();
    expect(renameContext(panel(), { kind: 'mode', collection: 'c1', id: 'nope' })).toBeNull();
    expect(renameContext(panel(), { kind: 'mode', collection: 'nope', id: 'm1' })).toBeNull();
  });
});

describe('checkRename', () => {
  const v = { current: 'color/bg', others: ['color/ink'] };
  it('takes a new name and sends it trimmed', () => {
    const c = checkRename('variable', ' color/surface ', v);
    expect(c).toEqual({ name: 'color/surface', error: null, warning: null, unchanged: false });
  });
  it('is a quiet cancel for the name it has now, as typed or with spaces around it: nothing is to be sent', () => {
    expect(checkRename('variable', 'color/bg', v).unchanged).toBe(true);
    expect(checkRename('variable', '  color/bg ', v).unchanged).toBe(true);
    expect(checkRename('variable', 'color/bg', v).error).toBeNull();
  });
  it('does not count the own name as taken, but refuses the name of another variable of the collection', () => {
    expect(checkRename('variable', 'color/ink', v).error).toMatch(/already has this name/i);
    expect(checkRename('variable', 'color/ink', v).unchanged).toBe(false);
  });
  it('refuses what an addition refuses: the rule of the kind, and hidden or direction characters', () => {
    expect(checkRename('variable', 'Color/Bg', v).error).toMatch(/lowercase/i);
    expect(checkRename('collection', 'a--b', { current: 'theme', others: [] }).error).toMatch(/lowercase/i);
    for (const bad of ['da‮rk', 'da​rk', 'darkㅤ', 'a b']) {
      expect(checkRename('mode', bad, { current: 'dark', others: [] }).error, JSON.stringify(bad)).toMatch(/control, direction or invisible/i);
    }
    expect(checkRename('variable', '', v).error).toMatch(/name/i);
  });
  it('keeps the limits of an addition: 256 characters for a variable, 64 for a mode', () => {
    expect(checkRename('variable', 'a'.repeat(NAME_MAX + 1), v).error).toContain(String(NAME_MAX));
    expect(checkRename('variable', 'a'.repeat(NAME_MAX), v).error).toBeNull();
    const m = { current: 'dark', others: [] };
    expect(checkRename('mode', 'x'.repeat(LABEL_MAX + 1), m).error).toContain(String(LABEL_MAX));
    expect(checkRename('mode', 'x'.repeat(LABEL_MAX), m).error).toBeNull();
  });
  it('warns, and does not refuse, a collection or a mode that takes the name of another', () => {
    const c = checkRename('collection', 'brand', { current: 'theme', others: ['brand'] });
    expect(c.error).toBeNull();
    expect(c.warning).toMatch(/already exists in this file/i);
    const m = checkRename('mode', 'Light', { current: 'dark', others: ['light'] });
    expect(m.error).toBeNull();
    expect(m.warning).toMatch(/already exists in this collection/i);
  });
  it('lets a mode that holds hidden characters be left as it is, and refuses it once it is edited and still holds them', () => {
    const odd = { current: 'da‮rk', others: [] };
    expect(checkRename('mode', 'da‮rk', odd).unchanged).toBe(true);
    expect(checkRename('mode', 'da‮rk2', odd).error).toMatch(/invisible/i);
    expect(checkRename('mode', 'dark', odd)).toMatchObject({ error: null, unchanged: false });
  });
});

describe('a name that changed while the field was open', () => {
  // The field opened on color/bg; another actor renamed it to color/surface meanwhile.
  const now = { current: 'color/surface', others: ['color/ink'] };
  it('is a quiet cancel when the text is still the one the field opened with: it must not undo the other actor', () => {
    const c = checkRename('variable', 'color/bg', now, 'color/bg');
    expect(c.unchanged).toBe(true);
    expect(c.error).toBeNull();
    expect(checkRename('variable', ' color/bg ', now, 'color/bg').unchanged).toBe(true);
  });
  it('is still a change for any other text, and the new name of the other actor is unchanged', () => {
    expect(checkRename('variable', 'color/brand', now, 'color/bg')).toMatchObject({ unchanged: false, error: null, name: 'color/brand' });
    expect(checkRename('variable', 'color/surface', now, 'color/bg').unchanged).toBe(true);
  });
  it('takes the opening text as the current name when none is given', () => {
    expect(checkRename('variable', 'color/surface', now).unchanged).toBe(true);
    expect(checkRename('variable', 'color/bg', now).unchanged).toBe(false);
  });
  it('says so in words, with the name as it is drawn, and says nothing while the name is the one it opened with', () => {
    expect(movedWords('color/bg', now)).toBe('This name changed to “color/surface” while you were editing.');
    expect(movedWords('da\u202erk', { current: 'Night\u200b', others: [] })).toBe('This name changed to “Night” while you were editing.');
    expect(movedWords('color/surface', now)).toBeNull();
  });
});

describe('the field around a rename', () => {
  const a = { kind: 'variable', id: 'v1' } as const;
  const b = { kind: 'collection', id: 'c1' } as const;
  const made = { error: null, status: 'Renamed.' };
  it('closes when its own rename was made, and asks for the focus back to the button', () => {
    const r = answerRename(sendRename(openRename(a)), made);
    expect(r.flow).toEqual(NO_RENAME);
    expect(r.refocus).toEqual(a);
  });
  it('stays open, with nothing to refocus, when its rename was refused', () => {
    const sent = sendRename(openRename(a));
    expect(answerRename(sent, { error: 'No.', status: null })).toEqual({ flow: { field: a, sent: null }, refocus: null });
  });
  it('ignores an answer when nothing was sent: a change of another actor must not close the field', () => {
    const open = openRename(a);
    expect(answerRename(open, made)).toEqual({ flow: open, refocus: null });
  });
  it('forgets what was sent whenever the field closes, so an old marker cannot close a later field', () => {
    // A rename was sent, the field was closed another way (an addition, a removal question, another collection), a new field was opened.
    const later = openRename(b);
    expect(later.sent).toBeNull();
    expect(answerRename(later, made).flow).toEqual(later);
    expect(closeRename()).toEqual(NO_RENAME);
    expect(sendRename(NO_RENAME)).toEqual(NO_RENAME);
  });
  it('does not take an answer for another field than the one that was sent', () => {
    const odd = { field: b, sent: a };
    expect(answerRename(odd, made)).toEqual({ flow: odd, refocus: null });
  });
});

describe('the words of a rename', () => {
  it('say what was renamed to what, the names as they are drawn', () => {
    expect(renamedWords('variable', 'color/bg', 'color/surface')).toBe('Renamed “color/bg” to “color/surface”.');
    expect(renamedWords('collection', 'theme', 'brand')).toBe('Renamed the collection “theme” to “brand”.');
    expect(renamedWords('mode', 'da‮rk', 'Night')).toBe('Renamed the mode “dark” to “Night”.');
    expect(renamedWords('mode', 'ㅤ', 'x')).toBe('Renamed the mode “(no name)” to “x”.');
  });
  it('say once what a rename does to the export, and what stays (docs/tokens-and-variables.md 9.6)', () => {
    expect(RENAME_HINT).toMatch(/next export uses the new name/i);
    expect(RENAME_HINT).toMatch(/code that uses the old name/i);
    expect(RENAME_HINT).toMatch(/bindings and aliases/i);
    expect(renameHint('variable', false)).toBe(RENAME_HINT);
    expect(renameHint('collection', false)).toBe(RENAME_HINT);
  });
  it('say for a mode only what the export does with it: a later mode is in the export under its name, the first one is not named at all', () => {
    // crates/features/export/export/src/vars.rs: the first mode is the :root rule, only the others get data-mode-<collection>="<mode>".
    const later = renameHint('mode', false);
    expect(later).toMatch(/next export uses the new name for this mode/i);
    expect(later).toMatch(/bindings and aliases keep working/i);
    const first = renameHint('mode', true);
    expect(first).toMatch(/first mode is the default/i);
    expect(first).toMatch(/not part of the exported names/i);
    expect(first).toMatch(/bindings and aliases keep working/i);
    expect(first).not.toMatch(/next export uses/i);
    expect(later).not.toMatch(/old name has to change/i);
    expect(first).not.toMatch(/old name has to change/i);
  });
  it('are the words of the table for a thing that was removed while the field was open', () => {
    expect(goneWords(panel(), { kind: 'variable', id: 'nope' })).toBe('That variable no longer exists.');
    expect(goneWords(panel(), { kind: 'collection', id: 'nope' })).toBe('That collection no longer exists.');
    expect(goneWords(panel(), { kind: 'mode', collection: 'c1', id: 'nope' })).toBe('That mode no longer exists.');
    // A mode whose collection went is gone with it.
    expect(goneWords(panel(), { kind: 'mode', collection: 'nope', id: 'm1' })).toBe('That collection no longer exists.');
  });
});

describe('the answer of the app layer to a rename', () => {
  const refused = (code: string) => ({ type: 'refused', code }) as Event;
  const rejected = (code: string) => ({ type: 'opRejected', key: 'k', code, node: null, retryable: false }) as Event;
  const changed = { type: 'documentChanged' } as Event;
  const sent = renameVariableAction('v1', 'color/ink');
  it('announces the change once the document changed, and no refusal', () => {
    const r = reactTo([changed], 'Renamed “a” to “b”.', sent);
    expect(r).toMatchObject({ error: null, status: 'Renamed “a” to “b”.', added: null });
  });
  it('words every refusal a rename can come back with, from the table, naming no code', () => {
    for (const code of ['E_INVALID_NAME', 'E_DUPLICATE_NAME', 'E_TOO_LONG', 'E_VARIABLE_NOT_FOUND', 'E_COLLECTION_NOT_FOUND', 'E_MODE_NOT_FOUND']) {
      for (const e of [refused(code), rejected(code)]) {
        const r = reactTo([e], 'Renamed.', sent);
        expect(r.error, code).not.toBeNull();
        expect(r.error, code).not.toContain(code);
        expect(r.error, code).not.toMatch(/^The (change was not made|server refused)/);
        expect(r.status, code).toBeNull();
      }
    }
    expect(reactTo([refused('E_DUPLICATE_NAME')], null, sent).error).toMatch(/already has that name/i);
    expect(reactTo([refused('E_VARIABLE_NOT_FOUND')], null, sent).error).toMatch(/no longer exists/i);
  });
});
