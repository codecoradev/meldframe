import type { ManageLimits, RenameField } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { nameError, renamedWords } from './manage';
import { RENAME_HINT, isEditing, movedWords, renameHint, renameNote } from './rename';

// What a rename is checked against, which text is no change, what a save sends and what happens to a field when another actor
// changes or removes its target are the app layer's (crates/core/meldframe-variables-manage-tests, manage_rename.rs); these are the words.
describe('the words of a rename', () => {
  it('say what was renamed to what, the names as they are drawn', () => {
    expect(renamedWords({ entity: 'variable', from: 'color/bg', to: 'color/page' })).toBe('Renamed “color/bg” to “color/page”.');
    expect(renamedWords({ entity: 'collection', from: 'theme', to: 'brand' })).toBe('Renamed the collection “theme” to “brand”.');
    expect(renamedWords({ entity: 'mode', from: 'dark', to: '' })).toBe('Renamed the mode “dark” to “(no name)”.');
  });
  it('say once what a rename does to the export, and what stays (docs/tokens-and-variables.md 9.6)', () => {
    expect(RENAME_HINT).toMatch(/next export uses the new name/);
    expect(RENAME_HINT).toMatch(/Bindings and aliases keep working/);
    expect(renameHint('variable', false)).toBe(RENAME_HINT);
    expect(renameHint('collection', false)).toBe(RENAME_HINT);
  });
  it('say for a mode only what the export does with it: a later mode is in the export under its name, the first one is not named at all', () => {
    expect(renameHint('mode', false)).toMatch(/uses the new name for this mode/);
    expect(renameHint('mode', false)).not.toBe(RENAME_HINT);
    expect(renameHint('mode', true)).toMatch(/not part of the exported names/);
    expect(renameHint('mode', true)).not.toBe(renameHint('mode', false));
  });
  it('say that a name changed meanwhile, with the name as it is drawn, and nothing while it is the one the field opened with', () => {
    expect(movedWords('color/bg', 'color/bg')).toBeNull();
    expect(movedWords('color/bg', 'color/page')).toBe('This name changed to “color/page” while you were editing.');
    expect(movedWords('color/bg', '')).toBe('This name changed to “(no name)” while you were editing.');
  });
});

describe('the line under the field of a rename', () => {
  const limits: ManageLimits = { nameMax: 256, modeNameMax: 64, collections: 32, modes: 16, variables: 5000 };
  const field = (over: Partial<RenameField> = {}): RenameField => ({ target: { kind: 'variable', id: 'v1' }, opened: 'color/bg', current: 'color/bg', text: 'color/bg', check: { name: 'color/bg' }, unchanged: true, first: false, ...over });
  it('says what a rename does to the export, with the rule of a name for a collection and a variable', () => {
    expect(renameNote('variable', field(), limits)).toEqual({ tone: 'hint', text: `Use lowercase letters, digits and single hyphens, with / between the parts, such as color/brand-500. ${RENAME_HINT}` });
    expect(renameNote('mode', field({ first: true }), limits)).toEqual({ tone: 'hint', text: renameHint('mode', true) });
  });
  it('says a refusal only for text that is a change: text that is no change shows the hint', () => {
    const bad = field({ text: 'Color', check: { name: 'Color', error: 'notPath' }, unchanged: false });
    expect(renameNote('variable', bad, limits)).toEqual({ tone: 'error', text: nameError('variable', 'notPath', limits) });
    expect(renameNote('variable', { ...bad, unchanged: true }, limits).tone).toBe('hint');
  });
  it('says a name that exists already as a warning', () => {
    expect(renameNote('mode', field({ text: 'dark', check: { name: 'dark', warning: 'same' }, unchanged: false }), limits).tone).toBe('warning');
  });
});

describe('whether the field of a rename is open for a thing of a collection', () => {
  const field = (target: RenameField['target']): RenameField => ({ target, opened: 'a', current: 'a', text: 'a', check: { name: 'a' }, unchanged: true, first: false });
  it('is true only for the target the field was opened for, a mode in its own collection', () => {
    const f = field({ kind: 'variable', id: 'v1' });
    expect(isEditing(f, 'c1', 'variable', 'v1')).toBe(true);
    expect(isEditing(f, 'c1', 'variable', 'v2')).toBe(false);
    expect(isEditing(f, 'c1', 'collection', 'v1')).toBe(false);
    expect(isEditing(null, 'c1', 'variable', 'v1')).toBe(false);
    const m = field({ kind: 'mode', collection: 'c1', id: 'm1' });
    expect(isEditing(m, 'c1', 'mode', 'm1')).toBe(true);
    expect(isEditing(m, 'c2', 'mode', 'm1')).toBe(false);
  });
});
