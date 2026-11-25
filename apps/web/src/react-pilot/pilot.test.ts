import { describe, expect, it } from 'vitest';
import { fixtureItems } from './fixture';
import { pilotPage, reactFor } from './pilot';

describe('the switch of a React panel (S2)', () => {
  it('names the panels that draw with React, and leaves every other page Svelte', () => {
    expect(reactFor('export', '?react=export')).toBe(true);
    expect(reactFor('export', '?canvaslayout=0&react=activity,export')).toBe(true);
    expect(reactFor('export', '')).toBe(false); // the default is Svelte
    expect(reactFor('export', '?react')).toBe(false); // the bare form is the state matrix page
    expect(reactFor('export', '?react=exports')).toBe(false);
    expect(reactFor('export', '?react=activity')).toBe(false);
  });
  it('does not turn the editor into the state matrix page', () => {
    expect(pilotPage('?react=export', false)).toBeNull();
  });
});

describe('the switch of the React pilot', () => {
  it('draws the React state matrix only when the page asks for it with `react`', () => {
    expect(pilotPage('?states&react', false)).toBe('states');
    expect(pilotPage('?react&states', false)).toBe('states');
    expect(pilotPage('?states', false)).toBeNull(); // the default page stays Svelte
    expect(pilotPage('?react', false)).toBeNull();
    expect(pilotPage('', false)).toBeNull();
    expect(pilotPage('?layout&react', false)).toBeNull();
  });
  it('offers the overlay fixtures in development only', () => {
    expect(pilotPage('?overlays=svelte', true)).toBe('overlays-svelte');
    expect(pilotPage('?overlays=react', true)).toBe('overlays-react');
    expect(pilotPage('?overlays=react', false)).toBeNull();
    expect(pilotPage('?overlays=other', true)).toBeNull();
    expect(pilotPage('?overlays', true)).toBeNull();
  });
});

describe('the overlay fixture rows', () => {
  it('have a disabled row between two enabled ones, and one that opens a dialog', () => {
    const picked: string[] = [];
    const rows = fixtureItems((id) => picked.push(id));
    expect(rows.map((r) => [r.id, !!r.disabled, !!r.opensDialog])).toEqual([
      ['alpha', false, false],
      ['bravo', true, false],
      ['charlie', false, false],
      ['dialog', false, true],
    ]);
    rows[2]!.onSelect();
    expect(picked).toEqual(['charlie']);
  });
});

describe('the switch of the palette and the shortcut sheet (S4)', () => {
  it('is the value `palette`, alone or in a list, and leaves the layout page and the editor Svelte without it', () => {
    expect(reactFor('palette', '?layout&react=palette')).toBe(true);
    expect(reactFor('palette', '?react=export,palette')).toBe(true);
    expect(reactFor('palette', '?layout')).toBe(false);
    expect(reactFor('palette', '?react=export')).toBe(false);
    expect(reactFor('palette', '?react=palettes')).toBe(false);
  });
  it('does not turn the layout page into the state matrix page', () => {
    expect(pilotPage('?layout&react=palette', false)).toBeNull();
  });
});
