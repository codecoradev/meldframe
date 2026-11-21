import { readFileSync } from 'node:fs';
import type { Node } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { isControl } from './edit';
import { commitText, editKeyTarget, textBoxStyle, textTarget } from './textedit';

const node = (id: string, over: Partial<Node> = {}): Node =>
  ({ id, name: id, type: 'rect', parentId: null, x: 0, y: 0, w: 100, h: 50, text: '', fontSize: 16, ...over }) as Node;
const text = (id: string, t = 'Hello') => node(id, { type: 'text', text: t, fontSize: 20 });

describe('textTarget', () => {
  const nodes = [text('t'), node('r'), text('u')];
  it('is the node when it is a text node', () => expect(textTarget('t', nodes)?.id).toBe('t'));
  it('is null for another kind of node or an id that is gone', () => {
    expect(textTarget('r', nodes)).toBeNull();
    expect(textTarget('nope', nodes)).toBeNull();
  });
});

describe('editKeyTarget (Enter on a selected text node)', () => {
  const nodes = [text('t'), node('r'), text('u')];
  const enter = { key: 'Enter', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false };
  it('opens the editor for exactly one selected text node', () => {
    expect(editKeyTarget(enter, ['t'], nodes)).toBe('t');
    expect(editKeyTarget(enter, ['t', 'u'], nodes)).toBeNull();
    expect(editKeyTarget(enter, ['r'], nodes)).toBeNull();
    expect(editKeyTarget(enter, [], nodes)).toBeNull();
  });
  it('ignores other keys and Enter with a modifier', () => {
    expect(editKeyTarget({ ...enter, key: 'a' }, ['t'], nodes)).toBeNull();
    expect(editKeyTarget({ ...enter, ctrlKey: true }, ['t'], nodes)).toBeNull();
    expect(editKeyTarget({ ...enter, shiftKey: true }, ['t'], nodes)).toBeNull();
  });
});

describe('commitText', () => {
  it('is one editText action for a changed text', () => {
    expect(commitText('t', 'Hello', 'Hello world')).toEqual({ type: 'editText', id: 't', text: 'Hello world' });
  });
  it('is nothing when the text did not change, so no empty undo step is made', () => {
    expect(commitText('t', 'Hello', 'Hello')).toBeNull();
  });
  it('keeps an emptied text (the node stays, its text is empty)', () => {
    expect(commitText('t', 'Hello', '')).toEqual({ type: 'editText', id: 't', text: '' });
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
    const n = [{ id: 't', type: 'text', name: 't', parentId: null } as never];
    expect(editKeyTarget({ key: 'Enter', ctrlKey: false, metaKey: false, shiftKey: false, altKey: false }, ['t'], n, el('BUTTON'))).toBeNull();
    expect(editKeyTarget({ key: 'Enter', ctrlKey: false, metaKey: false, shiftKey: false, altKey: false }, ['t'], n, el('BODY'))).toBe('t');
    expect(editKeyTarget({ key: 'Enter', ctrlKey: false, metaKey: false, shiftKey: false, altKey: false }, ['t'], n)).toBe('t');
  });
});
