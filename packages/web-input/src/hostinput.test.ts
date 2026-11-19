import { describe, expect, it } from 'vitest';
import { HOME } from '@meldframe/web-canvas/camera';
import { attachInput, type InputHooks } from './hostinput';

class FakeHost extends EventTarget {
  style = { cursor: '' };
  focused = 0;
  captured: number[] = [];
  getBoundingClientRect = () => ({ left: 10, top: 20 });
  focus = () => void this.focused++;
  setPointerCapture = (id: number) => void this.captured.push(id);
  fire(type: string, props: Record<string, unknown> = {}) {
    const e = Object.assign(new Event(type, { cancelable: true }), props);
    this.dispatchEvent(e);
    return e;
  }
}

const mods = { shiftKey: false, altKey: false, ctrlKey: false, metaKey: false };

function setup(over: Partial<InputHooks> = {}) {
  const host = new FakeHost();
  const calls: string[] = [];
  const log = (name: string) => (...a: unknown[]) => {
    calls.push(`${name} ${JSON.stringify(a)}`);
  };
  const state = { cursor: 'default', keyHandled: false };
  const hooks: InputHooks = {
    interaction: {
      down: log('down'), move: log('move'), up: log('up'), cancel: log('cancel'), leave: log('leave'), wheel: log('wheel'),
      key: (k) => (log('key')(k), state.keyHandled),
      get cursor() { return state.cursor; },
    },
    tool: () => 'move',
    camera: () => ({ ...HOME }),
    hit: (x, y) => (x === 10 && y === 20 ? 'n1' : null),
    schedule: log('schedule'),
    dropDraft: log('dropDraft'),
    settleSoon: log('settleSoon'),
    activate: log('activate'),
    ...over,
  };
  const detach = attachInput(host, hooks);
  return { host, calls, state, detach };
}

describe('attachInput', () => {
  it('a press focuses and captures the pointer, then reports it relative to the host', () => {
    const { host, calls } = setup();
    host.fire('pointerdown', { pointerId: 7, clientX: 15, clientY: 30, button: 0, pointerType: 'mouse', ...mods });
    expect(host.focused).toBe(1);
    expect(host.captured).toEqual([7]);
    expect(calls[0]).toBe(`down ${JSON.stringify([{ id: 7, x: 5, y: 10, button: 0, pointerType: 'mouse', mods: { shift: false, alt: false, ctrl: false, meta: false } }])}`);
    expect(calls).toContain('schedule []');
  });

  it('a pointer that cannot be captured still works', () => {
    const { host, calls } = setup();
    host.setPointerCapture = () => {
      throw new Error('gone');
    };
    host.fire('pointerdown', { pointerId: 1, clientX: 10, clientY: 20, button: 0, pointerType: 'touch', ...mods });
    expect(calls[0]).toMatch(/^down /);
  });

  it('move and up go to the interaction and the cursor follows it', () => {
    const { host, calls, state } = setup();
    state.cursor = 'grabbing';
    host.fire('pointermove', { pointerId: 1, clientX: 10, clientY: 20, button: 0, pointerType: 'mouse', ...mods });
    expect(host.style.cursor).toBe('grabbing');
    host.fire('pointerup', { pointerId: 1, clientX: 10, clientY: 20, button: 0, pointerType: 'mouse', ...mods });
    expect(calls.filter((c) => /^(move|up) /.test(c))).toHaveLength(2);
  });

  it('a cancelled pointer drops the draft; leaving only tells the interaction', () => {
    const { host, calls } = setup();
    host.fire('pointercancel');
    expect(calls).toEqual(['cancel []', 'dropDraft []', 'schedule []']);
    calls.length = 0;
    host.fire('pointerleave');
    expect(calls).toEqual(['leave []']);
  });

  it('a double click activates the node under the pointer with the move tool only', () => {
    const { host, calls } = setup();
    host.fire('dblclick', { clientX: 20, clientY: 40 });
    expect(calls).toEqual(['activate ["n1"]']);
    calls.length = 0;
    host.fire('dblclick', { clientX: 99, clientY: 99 });
    expect(calls).toEqual([]);
    const other = setup({ tool: () => 'rect' });
    other.host.fire('dblclick', { clientX: 20, clientY: 40 });
    expect(other.calls).toEqual([]);
  });

  it('a wheel is not left to scroll the page and the camera settles soon after', () => {
    const { host, calls } = setup();
    const e = host.fire('wheel', { clientX: 11, clientY: 22, deltaX: 1, deltaY: 2, deltaMode: 0, ctrlKey: true, metaKey: false });
    expect(e.defaultPrevented).toBe(true);
    expect(calls).toEqual([
      `wheel ${JSON.stringify([{ x: 1, y: 2, deltaX: 1, deltaY: 2, deltaMode: 0, ctrlKey: true, metaKey: false }])}`,
      'settleSoon []',
    ]);
  });

  it('a key the interaction handles is not left to the browser, and the draft goes unless drawing', () => {
    const { host, calls, state } = setup();
    state.keyHandled = true;
    const e = host.fire('keydown', { key: ' ', code: 'Space', ...mods });
    expect(e.defaultPrevented).toBe(true);
    expect(calls).toContain('dropDraft []');
    calls.length = 0;
    state.cursor = 'crosshair';
    host.fire('keyup', { key: 'a', code: 'KeyA', ...mods });
    expect(calls.some((c) => c.startsWith('dropDraft'))).toBe(false);
    expect(calls[0]).toContain('"down":false');
  });

  it('blur releases the space key', () => {
    const { host, calls } = setup();
    host.fire('blur');
    expect(calls[0]).toBe(`key ${JSON.stringify([{ key: ' ', down: false, mods: { shift: false, alt: false, ctrl: false, meta: false } }])}`);
  });

  it('the context menu is suppressed', () => {
    const { host } = setup();
    expect(host.fire('contextmenu').defaultPrevented).toBe(true);
  });

  it('detaching removes every listener', () => {
    const { host, calls, detach } = setup();
    detach();
    host.fire('pointermove', { pointerId: 1, clientX: 0, clientY: 0, button: 0, pointerType: 'mouse', ...mods });
    host.fire('keydown', { key: 'a', ...mods });
    host.fire('wheel', { clientX: 0, clientY: 0, deltaX: 0, deltaY: 0, deltaMode: 0, ctrlKey: false, metaKey: false });
    expect(calls).toEqual([]);
  });
});
