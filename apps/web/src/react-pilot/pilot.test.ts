import { describe, expect, it } from 'vitest';
import { fixtureItems } from './fixture';
import { pilotPage } from './pilot';

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
