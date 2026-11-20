import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import PaletteBody from './PaletteBody.svelte';
import { paletteEntries } from './palette';

const entries = paletteEntries({
  mac: false, layersOpen: true, inspectorOpen: false, canUndo: false, canRedo: false, canDelete: true, canExport: true, canVariant: true,
  on: { tool() {}, undo() {}, redo() {}, remove() {}, zoom() {}, preset() {}, layers() {}, inspector() {}, exportFrame() {}, variant() {}, help() {} },
});
const html = () => render(PaletteBody as never, { props: { entries, onRun: () => {}, onClose: () => {} } as never }).body.replace(/<!--.*?-->/g, '');

describe('the body of the palette, as it first appears', () => {
  it('is a combobox over a listbox, named, with the first entry that can run highlighted', () => {
    const out = html();
    expect(out).toMatch(/<input[^>]*role="combobox"[^>]*aria-controls="palette-list"/);
    expect(out).toMatch(/<input[^>]*aria-label="Search commands"/);
    expect(out).toContain('id="palette-list" role="listbox" aria-label="Commands"');
    expect(out).toMatch(/<input[^>]*aria-activedescendant="palette-option-tool-select"/);
    expect(out).toMatch(/id="palette-option-tool-select" role="option"[^>]*aria-selected="true"/);
    expect(out).toMatch(/id="palette-option-tool-frame" role="option"[^>]*aria-selected="false"/);
    expect(out.match(/role="option"/g)).toHaveLength(entries.length);
  });
  it('lists an entry that cannot run as disabled', () => {
    const out = html();
    expect(out).toMatch(/id="palette-option-undo" role="option"[^>]*aria-disabled="true"/);
    expect(out).toMatch(/id="palette-option-delete" role="option"[^>]*aria-disabled="false"/);
  });
  it('shows the name, the group, the size of a preset and the keys from the shortcut table', () => {
    const out = html();
    expect(out).toContain('New frame: A4');
    expect(out).toContain('595 × 842');
    expect(out).toContain('Ctrl+Shift+Z');
    expect(out).toContain('Hide layers panel');
    expect(out).toContain('Show right panel');
  });
  it('says nothing matches outside the listbox, which holds options only', () => {
    const out = render(PaletteBody as never, { props: { entries: [], onRun: () => {}, onClose: () => {} } as never }).body.replace(/<!--.*?-->/g, '');
    const list = out.match(/<div id="palette-list" role="listbox"[^>]*>(.*?)<\/div>/s)?.[1] ?? 'missing';
    expect(list).toBe('');
    expect(out).toContain('data-testid="palette-empty"');
    expect(out).toContain('No results');
    expect(html()).not.toContain('palette-empty');
  });
  it('is a status for readers: how many results', () => {
    expect(html()).toContain(`${entries.length} results`);
  });
});
