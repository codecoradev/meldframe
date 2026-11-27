import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { isControl } from './edit';
import { isEditKey, textBoxStyle } from './textedit';

describe('isEditKey (Enter, with no modifier)', () => {
  const enter = { key: 'Enter', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false };
  it('is Enter and nothing else', () => {
    expect(isEditKey(enter)).toBe(true);
    expect(isEditKey({ ...enter, key: 'a' })).toBe(false);
  });
  it('is not Enter with a modifier', () => {
    for (const m of ['ctrlKey', 'metaKey', 'shiftKey', 'altKey'] as const) expect(isEditKey({ ...enter, [m]: true })).toBe(false);
  });
});

describe('textBoxStyle', () => {
  it('puts the editor over the node on the screen and scales the font with the zoom', () => {
    const s = textBoxStyle({ x: 10, y: 20, zoom: 2 }, { x: 5, y: 6, w: 100, h: 30 }, 20);
    expect(s).toEqual({ left: 20, top: 32, width: 200, height: 60, fontSize: 40, fontWeight: 400, textAlign: 'left' });
  });
  it('carries the weight and the alignment the canvas draws the text with', () => {
    const s = textBoxStyle({ x: 0, y: 0, zoom: 1 }, { x: 0, y: 0, w: 100, h: 30 }, 20, { fontWeight: 700, textAlign: 'center' });
    expect(s).toMatchObject({ fontWeight: 700, textAlign: 'center' });
    expect(textBoxStyle({ x: 0, y: 0, zoom: 1 }, { x: 0, y: 0, w: 100, h: 30 }, 20, { fontWeight: 0 })).toMatchObject({ fontWeight: 400, textAlign: 'left' });
  });
  it('is applied by the field: its source sets the weight and the alignment from the style', () => {
    const view = readFileSync(new URL('../../../apps/web/src/TextEditor.svelte', import.meta.url), 'utf8');
    expect(view).toMatch(/style:font-weight=\{box\.fontWeight\}/);
    expect(view).toMatch(/style:text-align=\{box\.textAlign\}/);
    expect(view).toMatch(/style:font-size="\{box\.fontSize\}px"/);
  });
  it('never makes the field smaller than one line', () => {
    const s = textBoxStyle({ x: 0, y: 0, zoom: 1 }, { x: 0, y: 0, w: 10, h: 2 }, 20);
    expect(s.height).toBeGreaterThanOrEqual(20);
    expect(s.width).toBeGreaterThanOrEqual(10);
  });
});

describe('Enter on a control is the control\'s, not an edit', () => {
  const el = (tagName: string, role: string | null = null) => ({ tagName, getAttribute: (n: string) => (n === 'role' ? role : null) });
  it('knows a button, a link, a menu item, a tab and a radio', () => {
    for (const e of [el('BUTTON'), el('A'), el('DIV', 'menuitem'), el('DIV', 'tab'), el('DIV', 'radio'), el('SPAN', 'button')]) expect(isControl(e)).toBe(true);
    for (const e of [el('DIV'), el('CANVAS'), el('SECTION', 'region'), null]) expect(isControl(e)).toBe(false);
  });
  it('does not start an edit from a focused button, but still does from the page', () => {
    const enter = { key: 'Enter', ctrlKey: false, metaKey: false, shiftKey: false, altKey: false };
    expect(isEditKey(enter, el('BUTTON'))).toBe(false);
    expect(isEditKey(enter, el('BODY'))).toBe(true);
    expect(isEditKey(enter)).toBe(true);
  });
});
