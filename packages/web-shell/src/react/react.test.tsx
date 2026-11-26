// The React copies of the palette body, the palette and the shortcut sheet, checked as palette-body.test.ts and layout.test.ts
// check the Svelte ones: the markup of a server render, in Node. The dialog around the palette is a portal and the focus,
// the keys and Escape are the browser's: they are the job of packages/web-shell/scripts/palette-e2e.mjs and layout-e2e.mjs
// (MELDFRAME_UI=react), which ask the Svelte and the React page the same questions. The rules (palette, shortcuts, keys)
// are shared with the Svelte copies and keep their own tests. The last block compares the two copies' markup.
import { render } from 'svelte/server';
import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import PaletteBodySvelte from '../PaletteBody.svelte';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ShortcutHintSvelte from '../ShortcutHint.svelte';
import { paletteEntries } from '../palette';
import { Palette } from './Palette';
import { PaletteBody } from './PaletteBody';
import { ShortcutHint } from './ShortcutHint';

// The sheet writes its keys for the platform it runs on (`navigator.platform`), so a test that looks at the words pins it.
const pinPlatform = (platform: string) => vi.stubGlobal('navigator', { platform });
afterEach(() => vi.unstubAllGlobals());

const entries = paletteEntries({
  mac: false, layersOpen: true, inspectorOpen: false, canUndo: false, canRedo: false, canDelete: true, canExport: true, canVariant: true,
  on: { tool() {}, undo() {}, redo() {}, remove() {}, zoom() {}, preset() {}, layers() {}, inspector() {}, exportFrame() {}, variant() {}, help() {} },
});
const draw = (el: ReactElement) => renderToStaticMarkup(el);
const body = (over: Partial<Parameters<typeof PaletteBody>[0]> = {}) => draw(<PaletteBody entries={entries} onRun={() => {}} onClose={() => {}} {...over} />);
const sheet = (over: Partial<Parameters<typeof ShortcutHint>[0]> = {}) => draw(<ShortcutHint onClose={() => {}} {...over} />);

describe('the body of the palette, as it first appears', () => {
  it('is a combobox over a listbox, named, with the first entry that can run highlighted', () => {
    const out = body();
    expect(out).toMatch(/<input[^>]*role="combobox"[^>]*aria-controls="palette-list"/);
    expect(out).toMatch(/<input[^>]*aria-label="Search commands"/);
    expect(out).toContain('id="palette-list" role="listbox" aria-label="Commands"');
    expect(out).toMatch(/<input[^>]*aria-activedescendant="palette-option-tool-select"/);
    expect(out).toMatch(/id="palette-option-tool-select" role="option"[^>]*aria-selected="true"/);
    expect(out).toMatch(/id="palette-option-tool-frame" role="option"[^>]*aria-selected="false"/);
    expect(out.match(/role="option"/g)).toHaveLength(entries.length);
  });
  it('lists an entry that cannot run as disabled', () => {
    const out = body();
    expect(out).toMatch(/id="palette-option-undo" role="option"[^>]*aria-disabled="true"/);
    expect(out).toMatch(/id="palette-option-delete" role="option"[^>]*aria-disabled="false"/);
  });
  it('shows the name, the group, the size of a preset and the keys from the shortcut table', () => {
    const out = body();
    expect(out).toContain('New frame: A4');
    expect(out).toContain('595 × 842');
    expect(out).toContain('Ctrl+Shift+Z');
    expect(out).toContain('Hide layers panel');
    expect(out).toContain('Show right panel');
  });
  it('says nothing matches outside the listbox, which holds options only', () => {
    const out = body({ entries: [] });
    const list = out.match(/<div id="palette-list" role="listbox"[^>]*>(.*?)<\/div>/s)?.[1] ?? 'missing';
    expect(list).toBe('');
    expect(out).toContain('data-testid="palette-empty"');
    expect(out).toContain('No results');
    expect(body()).not.toContain('palette-empty');
  });
  it('is a status for readers: how many results', () => {
    expect(body()).toContain(`${entries.length} results`);
  });
  it('carries no highlight at all while no entry can run, and names no active row', () => {
    const none = entries.map((e) => ({ ...e, enabled: false }));
    const out = body({ entries: none });
    expect(out).not.toContain('aria-activedescendant');
    expect(out).not.toContain('aria-selected="true"');
  });
  it('keeps the field marked as the one that owns plain keys through the dialog, not itself, and gives it a name for the dialog to focus', () => {
    expect(body()).toContain('data-testid="palette-input"');
    expect(body()).not.toContain('data-owns-keys');
  });
});

describe('the palette, closed', () => {
  it('draws nothing while closed: the dialog is a portal that exists only while open', () => {
    expect(draw(<Palette open={false} onOpenChange={() => {}} entries={entries} />)).toBe('');
  });
});

describe('the shortcut sheet', () => {
  it('is a named dialog with the close button and a heading for each group', () => {
    pinPlatform('Linux x86_64');
    const out = sheet();
    expect(out).toMatch(/<div[^>]*role="dialog"[^>]*aria-label="Keyboard shortcuts"[^>]*data-testid="shortcut-hint"/);
    expect(out).toMatch(/<button[^>]*aria-label="Close shortcuts"/);
    for (const title of ['Tools', 'Edit', 'View', 'Selection', 'Panels and help']) expect(out, title).toContain(`>${title}</h2>`);
    expect(out.match(/<section[^>]*aria-labelledby="shortcut-group-/g)).toHaveLength(5);
    expect(out).toContain('id="shortcut-group-tools"');
    expect(out).toContain('The view keys work while the canvas has the focus.');
  });
  it('lists every group of shortcuts, written for the platform', () => {
    pinPlatform('Linux x86_64');
    const out = sheet();
    for (const text of ['Zoom in', 'Pan with the pointer', 'Ctrl+Shift+Z', 'Shift+1', 'Esc', 'Command palette', 'Ctrl+K']) expect(out, text).toContain(text);
  });
  it('writes the shortcuts the way the platform does', () => {
    pinPlatform('MacIntel');
    const mac = sheet();
    for (const text of ['⌥1', '⇧⌘Z']) expect(mac, text).toContain(text);
    for (const text of ['Alt+1', 'Ctrl+Shift+Z']) expect(mac, text).not.toContain(text);
    pinPlatform('Win32');
    const win = sheet();
    expect(win).toContain('Alt+1');
    expect(win).not.toContain('⌥1');
  });
  it('leaves out the row that needs auto layout while it is off, and has it while it is on', () => {
    pinPlatform('Linux x86_64');
    expect(sheet({ autoLayout: false })).not.toContain('Move one place in an auto layout frame');
    expect(sheet({ autoLayout: true })).toContain('Move one place in an auto layout frame');
    expect(sheet()).toContain('Move one place in an auto layout frame');
  });
});

// The two copies draw the same elements with the same classes in the same order (a class that is not here is a visible
// difference): the pixel check of the browser scripts finds the rest.
describe('the React copies against the Svelte ones', () => {
  const svelte = (component: unknown, props: Record<string, unknown>) => render(component as never, { props: props as never }).body.replace(/<!--.*?-->/g, '');
  const classes = (html: string) => [...html.matchAll(/class="([^"]*)"/g)].map((m) => m[1]!.split(/\s+/).filter(Boolean).sort().join(' '));
  // The ids a library makes for itself (the tooltip's) are not ours to compare.
  const attrs = (html: string, name: string) => [...html.matchAll(new RegExp(`${name}="([^"]*)"`, 'g'))].map((m) => m[1]!).filter((v) => !/^(bits|base-ui)-/.test(v));
  it('the palette body has the same classes, ids, roles and test ids', () => {
    const a = svelte(PaletteBodySvelte, { entries, onRun: () => {}, onClose: () => {} });
    const b = body();
    expect(classes(b)).toEqual(classes(a));
    for (const name of ['id', 'role', 'data-testid', 'data-id', 'aria-selected', 'aria-disabled', 'aria-label']) expect(attrs(b, name), name).toEqual(attrs(a, name));
  });
  it('the sheet has the same classes, roles and names', () => {
    pinPlatform('Linux x86_64');
    const a = svelte(TestHost, { of: ShortcutHintSvelte, props: { onClose: () => {}, autoLayout: true } });
    const b = sheet();
    expect(classes(b).filter((c) => !/size-|rounded-md|border-transparent/.test(c))).toEqual(classes(a).filter((c) => !/size-|rounded-md|border-transparent/.test(c)));
    for (const name of ['id', 'role', 'data-testid', 'aria-label', 'aria-labelledby']) expect(attrs(b, name), name).toEqual(attrs(a, name));
  });
});
