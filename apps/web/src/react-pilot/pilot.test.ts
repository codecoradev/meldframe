import { readdirSync, readFileSync } from 'node:fs';
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
  it('names the Layers panel with `layers`, alone or in a list, and not the word next to it (S5)', () => {
    expect(reactFor('layers', '?react=layers')).toBe(true);
    expect(reactFor('layers', '?react=export,layers')).toBe(true);
    expect(reactFor('layers', '?react=layer')).toBe(false);
    expect(reactFor('layers', '?react=export')).toBe(false);
    expect(pilotPage('?react=layers', false)).toBeNull();
  });
  it('does not turn the editor into the state matrix page', () => {
    expect(pilotPage('?react=export', false)).toBeNull();
  });
  it('names the Variables panel and its bind picker with one word, `variables`, and not the Layers panel (S5)', () => {
    expect(reactFor('variables', '?react=variables')).toBe(true);
    expect(reactFor('variables', '?react=layers,variables')).toBe(true);
    expect(reactFor('variables', '?react=layers')).toBe(false);
    expect(reactFor('variables', '?react=variable')).toBe(false);
    expect(reactFor('layers', '?react=variables')).toBe(false);
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

describe('the switch of the Properties panel (S6)', () => {
  it('is the value `properties`, alone or in a list, and not the Variables or Layers switch next to it', () => {
    expect(reactFor('properties', '?react=properties')).toBe(true);
    expect(reactFor('properties', '?react=layers,variables,properties')).toBe(true);
    expect(reactFor('properties', '?react=property')).toBe(false);
    expect(reactFor('properties', '?react=variables')).toBe(false);
    expect(reactFor('variables', '?react=properties')).toBe(false);
    expect(pilotPage('?react=properties', false)).toBeNull();
  });
});

// A React chunk that does not load must never leave a page without its panel: every switch file that loads React on demand draws the
// Svelte copy again in its `{:catch}`. The browser scripts block the chunks and ask (layers-e2e, variables-e2e, properties-e2e); this
// is the guard for a switch file that is added without it, which no script would reach until someone writes one.
describe('every switch file that loads React on demand falls back to Svelte', () => {
  const dir = new URL('.', import.meta.url);
  const switches = readdirSync(dir).filter((f) => f.endsWith('Switch.svelte'));
  it('finds the switch files', () => expect(switches.length).toBeGreaterThanOrEqual(8));
  for (const file of switches) {
    it(`${file} has a {:catch} that draws a component for each {#await}`, () => {
      // Without the comments: a `{:catch}` named in a comment is not one, and a catch that holds a comment only draws nothing.
      const source = readFileSync(new URL(file, dir), 'utf8').replace(/<!--[\s\S]*?-->/g, '');
      const awaits = source.match(/\{#await\b/g)?.length ?? 0;
      const catches = [...source.matchAll(/\{:catch\b[^}]*\}([\s\S]*?)\{\/await\}/g)].map((m) => m[1]!);
      expect(awaits).toBeGreaterThan(0);
      expect(catches).toHaveLength(awaits);
      // The fallback is a component (the Svelte copy of the panel), not an empty block.
      for (const block of catches) expect(block).toMatch(/<[A-Z]\w*/);
    });
  }
});

describe('the switch of the Activity panel (S8)', () => {
  it('is the value `activity`, alone or in a list, and not the word next to it', () => {
    expect(reactFor('activity', '?react=activity')).toBe(true);
    expect(reactFor('activity', '?react=export,activity')).toBe(true);
    expect(reactFor('activity', '?react=activities')).toBe(false);
    expect(reactFor('activity', '?react=export')).toBe(false);
    expect(reactFor('activity', '')).toBe(false);
    expect(pilotPage('?react=activity', false)).toBeNull();
  });
});

describe('the switch of the Import variables and Export variables dialogs (S10)', () => {
  it('is the value `io`, alone or in a list with the other Variables words, and not a word next to it', () => {
    expect(reactFor('io', '?react=io')).toBe(true);
    expect(reactFor('io', '?react=variables,manage,io')).toBe(true);
    expect(reactFor('io', '?react=ios')).toBe(false);
    expect(reactFor('io', '?react=variables')).toBe(false);
    expect(reactFor('io', '?react=manage')).toBe(false);
    expect(reactFor('variables', '?react=io')).toBe(false);
    expect(reactFor('io', '')).toBe(false);
    expect(pilotPage('?react=io', false)).toBeNull();
  });
  it('is read by both dialogs and by no other switch file', () => {
    const dir = new URL('.', import.meta.url);
    const readers = readdirSync(dir).filter((f) => f.endsWith('Switch.svelte') && readFileSync(new URL(f, dir), 'utf8').includes("reactFor('io'"));
    expect(readers.sort()).toEqual(['ExportVariablesDialogSwitch.svelte', 'ImportVariablesDialogSwitch.svelte']);
  });
});

describe('the switch of the canvas (S11)', () => {
  it('is the value `canvas`, alone or in a list, and not the word next to it; `strict` is its own word', () => {
    expect(reactFor('canvas', '?react=canvas')).toBe(true);
    expect(reactFor('canvas', '?react=layers,canvas,properties')).toBe(true);
    expect(reactFor('canvas', '?react=canvases')).toBe(false);
    expect(reactFor('canvas', '?react=layers')).toBe(false);
    expect(reactFor('canvas', '')).toBe(false);
    expect(reactFor('strict', '?react=canvas,strict')).toBe(true);
    expect(reactFor('strict', '?react=canvas')).toBe(false);
    expect(pilotPage('?react=canvas', false)).toBeNull();
  });
  it('answers every method the Svelte canvas has (the editor and the harness call them through bind:this)', () => {
    const names = (file: string) => [...readFileSync(new URL(file, import.meta.url), 'utf8').matchAll(/export function (\w+)/g)].map((m) => m[1]).sort();
    expect(names('./CanvasSwitch.svelte')).toEqual(names('../canvas/Canvas.svelte'));
    expect(names('../canvas/Canvas.svelte')).toHaveLength(7);
  });
});
