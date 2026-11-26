import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { barClear, fileItems, PANEL_INSET, reserved, rightWidth } from './layout';
import { layoutKey, paletteBlocked, readOverlays } from './keys';
import { composeProps } from '@meldframe/web-chrome/compose';
import { isMac, labels } from './platform';
import Layout from './Layout.svelte';

// The layout writes its shortcuts for the platform it runs on (`navigator.platform`, read when it renders), so a test that
// looks at the words pins the platform and does not take the host's: Node on macOS reports `MacIntel`.
const pinPlatform = (platform: string) => vi.stubGlobal('navigator', { platform });
afterEach(() => vi.unstubAllGlobals());

const key = (over: Partial<Parameters<typeof layoutKey>[0]>) => layoutKey({ key: '', code: '', altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...over });

describe('the keys of the layout', () => {
  it('Option+1 and Option+2 toggle the panels, by the physical key', () => {
    expect(key({ altKey: true, code: 'Digit1', key: '¡' })).toBe('layers');
    expect(key({ altKey: true, code: 'Digit2', key: '™' })).toBe('inspector');
    expect(key({ altKey: true, code: 'Digit3' })).toBeNull();
  });
  it('? opens the hint, and only without a modifier', () => {
    expect(key({ key: '?' })).toBe('help');
    expect(key({ key: '?', ctrlKey: true })).toBeNull();
    expect(key({ key: '?', metaKey: true })).toBeNull();
  });
  it('Ctrl+K and Command+K open the palette, whatever the case of the letter, and no other mix of modifiers does', () => {
    expect(key({ key: 'k', code: 'KeyK', ctrlKey: true })).toBe('palette');
    expect(key({ key: 'K', code: 'KeyK', metaKey: true })).toBe('palette');
    expect(key({ key: 'k', code: 'KeyK' })).toBeNull();
    expect(key({ key: 'k', code: 'KeyK', ctrlKey: true, shiftKey: true })).toBeNull();
    expect(key({ key: 'k', code: 'KeyK', ctrlKey: true, altKey: true })).toBeNull();
  });
  it('leaves tool keys and plain digits to the editor', () => {
    expect(key({ key: 'v', code: 'KeyV' })).toBeNull();
    expect(key({ key: '1', code: 'Digit1' })).toBeNull();
    expect(key({ altKey: true, ctrlKey: true, code: 'Digit1' })).toBeNull();
  });
});

describe('Ctrl+K over another overlay', () => {
  const el = (role: string, testId?: string) => ({ getAttribute: (n: string) => (n === 'role' ? role : n === 'data-testid' ? (testId ?? null) : null) });
  const root = (...els: ReturnType<typeof el>[]) => ({ querySelectorAll: () => els });

  it('reads the menus and dialogs that are open, by role and test id', () => {
    expect(readOverlays(root(el('menu'), el('dialog', 'export-dialog')))).toEqual([{ role: 'menu', id: null }, { role: 'dialog', id: 'export-dialog' }]);
    expect(readOverlays(root())).toEqual([]);
  });
  it('does not open over an open menu or a dialog of its own, and not while the Export dialog is open', () => {
    expect(paletteBlocked([{ role: 'menu', id: null }], false)).toBe(true);
    expect(paletteBlocked([{ role: 'dialog', id: 'export-dialog' }], false)).toBe(true);
    expect(paletteBlocked([], true)).toBe(true);
  });
  it('opens over nothing, over the shortcut sheet, and closes from itself', () => {
    expect(paletteBlocked([], false)).toBe(false);
    expect(paletteBlocked([{ role: 'dialog', id: 'shortcut-hint' }], false)).toBe(false);
    expect(paletteBlocked([{ role: 'dialog', id: 'palette' }], false)).toBe(false);
  });
});

describe('the layout rules', () => {
  it('the right card is 288 on the Design side and 360 on the Activity tab', () => {
    expect(rightWidth('design')).toBe(288);
    expect(rightWidth('activity')).toBe(360);
  });
  it('an open panel reserves its inset and width; a closed one nothing', () => {
    expect(reserved({ layers: true, inspector: true })).toEqual({ left: PANEL_INSET + 260, right: PANEL_INSET + 288 });
    expect(reserved({ layers: false, inspector: false })).toEqual({ left: 0, right: 0 });
  });
  it('the tool bar clears both open panels at 1440 and 1280 wide, even with the Activity width', () => {
    for (const w of [1440, 1280]) {
      expect(barClear(w, 480, { layers: true, inspector: true })).toBe(true);
      expect(barClear(w, 480, { layers: true, inspector: true, tab: 'activity' })).toBe(true);
    }
  });
  it('says no when a bar would touch a panel', () => {
    expect(barClear(1024, 480, { layers: true, inspector: true })).toBe(false);
    expect(barClear(1280, 800, { layers: true, inspector: true })).toBe(false);
  });
});

describe('the rows of the file menu', () => {
  const calls: string[] = [];
  const items = (o: { canExport: boolean; canVariant: boolean }) => fileItems({ ...o, onExport: () => calls.push('export'), onVariant: () => calls.push('variant') });

  it('has Export and Make a variant, with the test ids the browser checks use', () => {
    expect(items({ canExport: true, canVariant: true }).map((i) => [i.id, i.label, i.testId])).toEqual([
      ['export', 'Export…', 'export'],
      ['variant', 'Make a variant', 'make-variant'],
    ]);
  });

  it('disables each row when there is no frame for it', () => {
    expect(items({ canExport: false, canVariant: true }).map((i) => !!i.disabled)).toEqual([true, false]);
    expect(items({ canExport: true, canVariant: false }).map((i) => !!i.disabled)).toEqual([false, true]);
  });

  it('only Export opens a dialog, which is why the menu does not give the focus back to its button', () => {
    expect(items({ canExport: true, canVariant: true }).map((i) => !!i.opensDialog)).toEqual([true, false]);
  });

  it('a row calls the handler it was given', () => {
    calls.length = 0;
    const [exp, variant] = items({ canExport: true, canVariant: true });
    exp!.onSelect();
    variant!.onSelect();
    expect(calls).toEqual(['export', 'variant']);
  });
});

describe('the Layout', () => {
  const part = createRawSnippet(() => ({ render: () => '<i>x</i>' }));
  const named = (name: string) => createRawSnippet(() => ({ render: () => `<i>${name}</i>` }));
  const props = {
    fileName: 'file_dev', tool: 'select', onTool: () => {}, layersOpen: true, inspectorOpen: true, onToggleLayers: () => {}, onToggleInspector: () => {},
    canUndo: false, canRedo: true, onUndo: () => {}, onRedo: () => {}, onPreset: () => {}, canExport: false, onExport: () => {}, canVariant: false,
    onVariant: () => {}, zoomText: '100%', onZoom: () => {}, connection: { state: 'live', text: 'Connected' }, helpOpen: false, onHelp: () => {}, paletteOpen: false, onPalette: () => {}, canDelete: true, onDelete: () => {},
    canvasLayer: part, layersPanel: part, designPanel: named('design-x'), activityPanel: named('activity-x'), tab: 'design', onTab: () => {},
  };
  const html = (over: Record<string, unknown> = {}) => render(Layout as never, { props: { ...props, ...over } as never }).body;
  it('keeps the test ids the browser checks use', () => {
    const out = html();
    for (const id of ['toolbar', 'tool-select', 'tool-frame', 'tool-rectangle', 'tool-ellipse', 'tool-text', 'undo', 'redo', 'toggle-layers', 'toggle-properties', 'connection', 'zoom', 'tool', 'file-menu', 'frame-presets']) {
      expect(out, id).toContain(`data-testid="${id}"`);
    }
  });
  it('shows no Share button and no account', () => {
    expect(html()).not.toMatch(/Share|Account/);
  });
  it('the active tool is pressed, and undo is disabled while there is nothing to undo', () => {
    const out = html();
    expect(out).toMatch(/aria-label="Select"[^>]*aria-pressed="true"/);
    expect(out).toMatch(/<button[^>]*aria-label="Undo"[^>]*\sdisabled[=\s>]/);
    expect(out).not.toMatch(/<button[^>]*aria-label="Redo"[^>]*\sdisabled[=\s>]/);
  });
  it('the Ellipse tool sits between Rectangle and Text, is named for assistive technology and is pressed while chosen', () => {
    const out = html({ tool: 'ellipse' });
    expect(out).toMatch(/aria-label="Ellipse"[^>]*aria-pressed="true"/);
    expect(out).toMatch(/aria-label="Rectangle"[^>]*aria-pressed="false"/);
    const at = (label: string) => out.indexOf(`aria-label="${label}"`);
    expect(at('Rectangle')).toBeLessThan(at('Ellipse'));
    expect(at('Ellipse')).toBeLessThan(at('Text'));
  });
  it('closed panels become the two 52 px cards, each with the toggle that opens it', () => {
    const out = html({ layersOpen: false, inspectorOpen: false });
    expect(out).toContain('data-testid="top-left-card"');
    expect(out).toContain('data-testid="top-right-card"');
    expect(out).toMatch(/aria-label="Layers panel"[^>]*aria-pressed="false"/);
    expect(out).toMatch(/aria-label="Right panel"[^>]*aria-pressed="false"/);
    expect(out).not.toContain('data-testid="layers-card"');
  });
  it('the right card has the tabs Design and Activity, both available, each with its panel', () => {
    const out = html();
    expect(out).toContain('aria-label="Panel sections"');
    expect(out).toMatch(/id="right-tab-activity"[^>]*aria-controls="right-panel-activity"/);
    expect(out).not.toMatch(/id="right-tab-activity"[^>]*aria-disabled/);
    expect(out).toMatch(/aria-selected="true"[^>]*>Design/);
  });
  it('shows the panel of the selected tab only, named by its tab', () => {
    const design = html();
    expect(design).toContain('design-x');
    expect(design).not.toContain('activity-x');
    expect(design).toMatch(/id="right-panel-design" role="tabpanel" aria-labelledby="right-tab-design"/);
    const activity = html({ tab: 'activity' });
    expect(activity).toContain('activity-x');
    expect(activity).not.toContain('design-x');
    expect(activity).toMatch(/id="right-panel-activity" role="tabpanel" aria-labelledby="right-tab-activity"/);
    expect(activity).toMatch(/aria-selected="true"[^>]*>Activity/);
  });
  it('puts the count of batches to review on the Activity tab, and nothing when there are none', () => {
    expect(html({ activityBadge: 3 }).replace(/<!--.*?-->/g, '')).toMatch(/id="right-tab-activity"[^>]*>Activity<span[^>]*>3<span class="sr-only"> batches to review<\/span>/);
    expect(html({ activityBadge: 1 }).replace(/<!--.*?-->/g, '')).toMatch(/>1<span class="sr-only"> batch to review<\/span>/);
    expect(html({ activityBadge: 1 })).not.toContain('1 batches');
    expect(html({ activityBadge: 0 })).not.toContain('batches to review');
    expect(html()).not.toContain('batches to review');
  });
  it('the card is 288 wide on Design and 360 on Activity', () => {
    expect(html()).toMatch(/data-testid="right-card"[^>]*style="width: 288px/);
    expect(html({ tab: 'activity' })).toMatch(/data-testid="right-card"[^>]*style="width: 360px/);
  });
});

describe('what the Layout says to the keyboard and to readers', () => {
  const part = createRawSnippet(() => ({ render: () => '<i>x</i>' }));
  const props = {
    fileName: 'f', tool: 'select', onTool: () => {}, layersOpen: true, inspectorOpen: true, onToggleLayers: () => {}, onToggleInspector: () => {},
    canUndo: true, canRedo: true, onUndo: () => {}, onRedo: () => {}, onPreset: () => {}, canExport: true, onExport: () => {}, canVariant: true,
    onVariant: () => {}, zoomText: '100%', onZoom: () => {}, connection: { state: 'live', text: 'Connected' }, helpOpen: false, onHelp: () => {}, paletteOpen: false, onPalette: () => {}, canDelete: true, onDelete: () => {},
    canvasLayer: part, layersPanel: part, designPanel: part, activityPanel: part, tab: 'design', onTab: () => {},
  };
  const out = (over: Record<string, unknown> = {}) => render(Layout as never, { props: { ...props, ...over } as never }).body;
  it('every card, open or folded, carries the marker that keeps the plain keys for what is inside it', () => {
    expect(out().match(/data-owns-keys/g)).toHaveLength(2);
    expect(out({ layersOpen: false, inspectorOpen: false }).match(/data-owns-keys/g)).toHaveLength(2);
    expect(out()).toMatch(/<section[^>]*aria-label="Left panel"[^>]*data-owns-keys/);
    expect(out()).not.toMatch(/data-owns-keys[^>]*data-testid="toolbar"/);
  });
  it('a panel toggle is one control: a fixed name and its state, never a name that flips', () => {
    expect(out()).not.toMatch(/aria-label="(Show|Hide) /);
    expect(out().match(/aria-label="Layers panel"/g)).toHaveLength(2);
  });
  it('the tool echo is not announced at every change, and the tool bar is a group, not a toolbar', () => {
    expect(out()).toMatch(/<output[^>]*aria-live="off"[^>]*data-testid="tool"/);
    expect(out()).not.toContain('role="toolbar"');
    expect(out()).toMatch(/role="group" aria-label="Tools"/);
  });
  it('the zoom readout is the button of the zoom menu: the percentage is its text and its name says what it is', () => {
    const button = out().match(/<button[^>]*data-testid="zoom"[^>]*>[^<]*<\/button>/)?.[0] ?? '';
    expect(button).toMatch(/>100%<\/button>/);
    expect(button).toContain('aria-label="Zoom 100%"');
    expect(button).toContain('aria-haspopup="menu"');
    expect(out({ zoomText: '125%' })).toContain('aria-label="Zoom 125%"');
  });
  it('the sheet lists every group of shortcuts, written for the platform, and opens with the help button', () => {
    pinPlatform('Linux x86_64');
    const open = out({ helpOpen: true });
    for (const title of ['Tools', 'Edit', 'View', 'Selection', 'Panels and help']) expect(open, title).toContain(title);
    for (const text of ['Zoom in', 'Pan with the pointer', 'Ctrl+Shift+Z', 'Shift+1', 'Esc']) expect(open, text).toContain(text);
    expect(out()).not.toContain('data-testid="shortcut-hint"');
  });
  it('writes the shortcuts the way the platform does', () => {
    expect(isMac('MacIntel')).toBe(true);
    expect(isMac('Win32')).toBe(false);
    expect(isMac('Linux x86_64')).toBe(false);
    expect(labels(true)).toEqual({ layers: '⌥1', inspector: '⌥2', undo: '⌘Z', redo: '⇧⌘Z' });
    expect(labels(false).layers).toBe('Alt+1');
    pinPlatform('Win32');
    expect(out({ helpOpen: true })).toContain('Alt+1');
    expect(out({ helpOpen: true })).not.toContain('⌥1');
  });
  it('on a Mac the shortcut sheet uses the Option, Command and Shift symbols and not the words', () => {
    pinPlatform('MacIntel');
    const open = out({ helpOpen: true });
    for (const text of ['⌥1', '⇧⌘Z']) expect(open, text).toContain(text);
    for (const text of ['Alt+1', 'Ctrl+Shift+Z']) expect(open, text).not.toContain(text);
  });
  it('the same sheet says the words again once the platform is not a Mac', () => {
    pinPlatform('MacIntel');
    expect(out({ helpOpen: true })).toContain('⇧⌘Z');
    pinPlatform('Linux x86_64');
    expect(out({ helpOpen: true })).toContain('Ctrl+Shift+Z');
  });
});

describe('two triggers on one button', () => {
  it('chains the handlers of both and lets the later value win for the rest', () => {
    const seen: string[] = [];
    const merged = composeProps(
      { onpointerdown: () => seen.push('tooltip'), id: 'tip', 'data-state': 'closed' },
      { onpointerdown: () => seen.push('menu'), id: 'menu', 'aria-haspopup': 'menu' },
      { onclick: undefined },
    );
    (merged.onpointerdown as () => void)();
    expect(seen).toEqual(['tooltip', 'menu']);
    expect(merged.id).toBe('menu');
    expect(merged['data-state']).toBe('closed');
    expect(merged['aria-haspopup']).toBe('menu');
  });
  it('keeps the attachments, which are props with symbol keys (a menu is positioned by its trigger\'s)', () => {
    const anchor = Symbol('anchor');
    const tooltip = Symbol('tooltip');
    const fn = () => {};
    const merged = composeProps({ [tooltip]: fn }, { [anchor]: fn });
    expect(merged[anchor]).toBe(fn);
    expect(merged[tooltip]).toBe(fn);
  });
});
