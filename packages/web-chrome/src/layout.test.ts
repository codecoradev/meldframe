import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import { barClear, PANEL_INSET, reserved, rightWidth } from './layout';
import { layoutKey } from './keys';
import { composeProps } from './compose';
import { isMac, labels } from './platform';
import Layout from './Layout.svelte';

const key = (over: Partial<Parameters<typeof layoutKey>[0]>) => layoutKey({ key: '', code: '', altKey: false, ctrlKey: false, metaKey: false, ...over });

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
  it('leaves tool keys and plain digits to the editor', () => {
    expect(key({ key: 'v', code: 'KeyV' })).toBeNull();
    expect(key({ key: '1', code: 'Digit1' })).toBeNull();
    expect(key({ altKey: true, ctrlKey: true, code: 'Digit1' })).toBeNull();
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

describe('the Layout', () => {
  const part = createRawSnippet(() => ({ render: () => '<i>x</i>' }));
  const props = {
    fileName: 'file_dev', tool: 'select', onTool: () => {}, layersOpen: true, inspectorOpen: true, onToggleLayers: () => {}, onToggleInspector: () => {},
    canUndo: false, canRedo: true, onUndo: () => {}, onRedo: () => {}, onPreset: () => {}, canExport: false, onExport: () => {}, canVariant: false,
    onVariant: () => {}, zoomText: '100%', connection: { state: 'live', text: 'Connected' }, helpOpen: false, onHelp: () => {},
    canvasLayer: part, layersPanel: part, designPanel: part,
  };
  const html = (over: Record<string, unknown> = {}) => render(Layout as never, { props: { ...props, ...over } as never }).body;
  it('keeps the test ids the browser checks use', () => {
    const out = html();
    for (const id of ['toolbar', 'tool-select', 'tool-frame', 'tool-rectangle', 'tool-text', 'undo', 'redo', 'toggle-layers', 'toggle-properties', 'connection', 'zoom', 'tool', 'file-menu', 'frame-presets']) {
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
  it('closed panels become the two 52 px cards, each with the toggle that opens it', () => {
    const out = html({ layersOpen: false, inspectorOpen: false });
    expect(out).toContain('data-testid="top-left-card"');
    expect(out).toContain('data-testid="top-right-card"');
    expect(out).toMatch(/aria-label="Layers panel"[^>]*aria-pressed="false"/);
    expect(out).toMatch(/aria-label="Right panel"[^>]*aria-pressed="false"/);
    expect(out).not.toContain('data-testid="layers-card"');
  });
  it('the right card has the tabs Design and Activity, and Activity is not available yet', () => {
    const out = html();
    expect(out).toContain('aria-label="Panel sections"');
    expect(out).toMatch(/id="right-tab-activity"[^>]*aria-disabled="true"/);
    expect(out).not.toMatch(/id="right-tab-activity"[^>]*aria-controls/);
    expect(out).toMatch(/aria-selected="true"[^>]*>Design/);
  });
});

describe('what the Layout says to the keyboard and to readers', () => {
  const part = createRawSnippet(() => ({ render: () => '<i>x</i>' }));
  const props = {
    fileName: 'f', tool: 'select', onTool: () => {}, layersOpen: true, inspectorOpen: true, onToggleLayers: () => {}, onToggleInspector: () => {},
    canUndo: true, canRedo: true, onUndo: () => {}, onRedo: () => {}, onPreset: () => {}, canExport: true, onExport: () => {}, canVariant: true,
    onVariant: () => {}, zoomText: '100%', connection: { state: 'live', text: 'Connected' }, helpOpen: false, onHelp: () => {},
    canvasLayer: part, layersPanel: part, designPanel: part,
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
  it('the zoom and the tool echo are not announced at every change, and the tool bar is a group, not a toolbar', () => {
    expect(out()).toMatch(/<output[^>]*aria-live="off"[^>]*data-testid="zoom"/);
    expect(out()).toMatch(/<output[^>]*aria-live="off"[^>]*data-testid="tool"/);
    expect(out()).not.toContain('role="toolbar"');
    expect(out()).toMatch(/role="group" aria-label="Tools"/);
  });
  it('writes the shortcuts the way the platform does', () => {
    expect(isMac('MacIntel')).toBe(true);
    expect(isMac('Win32')).toBe(false);
    expect(isMac('Linux x86_64')).toBe(false);
    expect(labels(true)).toEqual({ layers: '⌥1', inspector: '⌥2', undo: '⌘Z', redo: '⇧⌘Z' });
    expect(labels(false).layers).toBe('Alt+1');
    expect(out({ helpOpen: true })).toContain('Alt+1');
    expect(out({ helpOpen: true })).not.toContain('⌥1');
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
