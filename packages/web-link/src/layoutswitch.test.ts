import { describe, expect, it, vi } from 'vitest';
import { LAYOUT_STORAGE_KEY, createLayoutSwitch, layoutChoice, layoutNotice, toggleWithNotice } from './layoutswitch';

/** A storage the test fills by hand; `broken` throws on every call, as a private window or blocked site data does. */
function store(initial: Record<string, string> = {}, broken = false) {
  const data = new Map(Object.entries(initial));
  const guard = () => { if (broken) throw new Error('blocked'); };
  const s = {
    getItem: (k: string) => { guard(); return data.get(k) ?? null; },
    setItem: (k: string, v: string) => { guard(); data.set(k, v); },
    data,
  };
  return s as typeof s & Storage;
}

describe('layoutChoice: whether the canvas draws laid-out boxes', () => {
  it('is on by default', () => {
    expect(layoutChoice('', () => store())).toBe(true);
    expect(layoutChoice('?tokens', () => store())).toBe(true);
  });
  it('is what the person chose last in this browser', () => {
    expect(layoutChoice('', () => store({ [LAYOUT_STORAGE_KEY]: 'off' }))).toBe(false);
    expect(layoutChoice('', () => store({ [LAYOUT_STORAGE_KEY]: 'on' }))).toBe(true);
    expect(layoutChoice('', () => store({ [LAYOUT_STORAGE_KEY]: 'garbage' }))).toBe(true);
  });
  it('is forced by ?canvaslayout=0 and ?canvaslayout=1, whatever was chosen', () => {
    const off = () => store({ [LAYOUT_STORAGE_KEY]: 'off' });
    const on = () => store({ [LAYOUT_STORAGE_KEY]: 'on' });
    expect(layoutChoice('?canvaslayout=1', off)).toBe(true);
    expect(layoutChoice('?tokens&canvaslayout=1', off)).toBe(true);
    expect(layoutChoice('?canvaslayout=0', on)).toBe(false);
    expect(layoutChoice('?canvaslayout=0', () => store())).toBe(false);
  });
  it('ignores any other value of the parameter', () => {
    for (const q of ['?canvaslayout', '?canvaslayout=true', '?canvaslayout=2', '?layout=0', '?x=canvaslayout=0']) expect(layoutChoice(q, () => store()), q).toBe(true);
  });
  it('is on when the storage cannot be had or read: the page renders without it', () => {
    expect(layoutChoice('', () => { throw new Error('no storage'); })).toBe(true);
    expect(layoutChoice('', () => store({}, true))).toBe(true);
  });
});

function rig(initial: boolean, storage: Storage | null = store()) {
  const engine = { setLayoutOnCanvas: vi.fn() };
  const stop = vi.fn();
  const installMeasurer = vi.fn(() => stop);
  const sw = createLayoutSwitch(engine, installMeasurer, initial, () => storage);
  return { engine, installMeasurer, stop, sw, storage };
}

describe('createLayoutSwitch', () => {
  it('starts on: the measurer is installed first, then the engine draws laid-out boxes', () => {
    const order: string[] = [];
    const engine = { setLayoutOnCanvas: vi.fn((on: boolean) => order.push(`flag ${on}`)) };
    const sw = createLayoutSwitch(engine, () => (order.push('measurer'), () => {}), true, () => store());
    expect(order).toEqual(['measurer', 'flag true']);
    expect(sw.on).toBe(true);
  });
  it('starts off: nothing is installed and the engine is not told (the stored boxes are what it draws)', () => {
    const r = rig(false);
    expect(r.installMeasurer).not.toHaveBeenCalled();
    expect(r.engine.setLayoutOnCanvas).not.toHaveBeenCalled();
    expect(r.sw.on).toBe(false);
  });
  it('turning on installs the measurer once, however often it is switched', () => {
    const r = rig(false);
    expect(r.sw.toggle()).toBe(true);
    expect(r.sw.toggle()).toBe(false);
    expect(r.sw.toggle()).toBe(true);
    expect(r.installMeasurer).toHaveBeenCalledTimes(1);
    expect(r.engine.setLayoutOnCanvas.mock.calls.map((c) => c[0])).toEqual([true, false, true]);
  });
  it('remembers each choice of the person in this browser', () => {
    const s = store();
    const r = rig(true, s);
    r.sw.toggle();
    expect(s.data.get(LAYOUT_STORAGE_KEY)).toBe('off');
    r.sw.toggle();
    expect(s.data.get(LAYOUT_STORAGE_KEY)).toBe('on');
    expect(layoutChoice('', () => s)).toBe(true);
  });
  it('does not remember the choice the page started with (a ?canvaslayout=0 link leaves the preference alone)', () => {
    const s = store({ [LAYOUT_STORAGE_KEY]: 'on' });
    rig(false, s);
    expect(s.data.get(LAYOUT_STORAGE_KEY)).toBe('on');
  });
  it('still switches when the storage throws or is missing', () => {
    for (const storage of [store({}, true), null]) {
      const r = rig(true, storage);
      expect(r.sw.toggle()).toBe(false);
      expect(r.engine.setLayoutOnCanvas).toHaveBeenLastCalledWith(false);
    }
    const throwing = createLayoutSwitch({ setLayoutOnCanvas: () => {} }, () => () => {}, true, () => { throw new Error('no storage'); });
    expect(throwing.toggle()).toBe(false);
  });
});

describe('the notice of the switch', () => {
  it('says what the switch is now, in the words of the entry', () => {
    expect(layoutNotice(true)).toBe('Auto layout on canvas: on');
    expect(layoutNotice(false)).toBe('Auto layout on canvas: off');
  });
  it('is said after each turn, once, with the state the switch has then', () => {
    const r = rig(true);
    const said: string[] = [];
    expect(toggleWithNotice(r.sw, (t) => said.push(t))).toBe(false);
    expect(toggleWithNotice(r.sw, (t) => said.push(t))).toBe(true);
    expect(said).toEqual(['Auto layout on canvas: off', 'Auto layout on canvas: on']);
  });
  it('is said even when the choice could not be remembered', () => {
    const r = rig(true, store({}, true));
    const said: string[] = [];
    toggleWithNotice(r.sw, (t) => said.push(t));
    expect(said).toEqual(['Auto layout on canvas: off']);
  });
});
