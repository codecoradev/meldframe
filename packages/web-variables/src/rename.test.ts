import { describe, expect, it } from 'vitest';
import { renamedWords } from './manage';
import { RENAME_HINT, movedWords, renameHint } from './rename';

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
