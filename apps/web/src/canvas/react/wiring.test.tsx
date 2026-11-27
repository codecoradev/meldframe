// The effects of the React canvas (`useCanvasDriver`, the part of Canvas.tsx that touches the driver), on a real React root in Node with a
// fake driver: a stand-in container and a probe that returns null, as react.test.tsx of web-chrome does it, because Node has no DOM for the
// markup (canvas.test.tsx renders that on the server). They ask what a render leaves in the driver: the callbacks of the latest render, the
// props of each render, the host, and what strict mode and an unmount do to them. What the driver does with them is driver.test.ts.
import { type ReactNode, StrictMode, act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import type { CanvasEvents } from '../api';
import { type CanvasDriver } from '../driver';
import { type DrawProps, useCanvasDriver } from './Canvas';

const view = { document: { activeElement: null, body: null }, HTMLIFrameElement: class {} };
const container = () =>
  ({
    nodeType: 1, nodeName: 'DIV', tagName: 'DIV', namespaceURI: 'http://www.w3.org/1999/xhtml',
    addEventListener() {}, removeEventListener() {},
    ownerDocument: { defaultView: view, addEventListener() {}, removeEventListener() {} },
  }) as unknown as HTMLElement;

/** A host element and a document that makes canvases, enough for `mountOn`. */
function dom() {
  const children: { className: string; remove(): void }[] = [];
  const host = { prepend: (el: (typeof children)[number]) => void children.unshift(el) } as unknown as HTMLElement;
  const doc = { createElement: () => { const el = { className: '', remove: () => void children.splice(children.indexOf(el), 1) }; return el; } };
  return { children, host, doc };
}

/** A driver that only keeps what it is given: `setProps` calls, its `events`, and each mount and stop. */
function fakeDriver() {
  const log: string[] = [];
  const driver = {
    events: {} as CanvasEvents,
    setProps: vi.fn(),
    mount: vi.fn(() => {
      log.push('mount');
      return () => void log.push('stop');
    }),
  };
  return { driver, log, as: driver as unknown as CanvasDriver };
}

const scene = { space: 'parent' as const, nodes: [] };
const draws = (over: Partial<DrawProps> = {}): DrawProps => ({ scene, selection: [], camera: { x: 0, y: 0, zoom: 1 }, touches: [], indicator: null, tool: 'move', theme: 'dark', ...over });

beforeAll(() => Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true, window: view }));
afterAll(() => {
  Reflect.deleteProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT');
  Reflect.deleteProperty(globalThis, 'window');
  Reflect.deleteProperty(globalThis, 'document');
});
afterEach(() => vi.restoreAllMocks());

/** Mounts a probe that calls the hook with what `current` holds, on a fresh root; `rerender` draws it again with new values. */
async function probe(d: ReturnType<typeof fakeDriver>, first: { draws: DrawProps; events: Omit<CanvasEvents, 'onStatus'> }, strict = false) {
  const els = dom();
  Object.assign(globalThis, { document: els.doc });
  const status = vi.fn();
  const hostRef = { current: els.host }; // a ref keeps its identity between renders, as useRef's does
  let current = first;
  const Probe = (): ReactNode => {
    useCanvasDriver(d.as, hostRef, current.draws, current.events, status);
    return null;
  };
  const root = createRoot(container());
  const ui = strict ? <StrictMode><Probe /></StrictMode> : <Probe />;
  await act(async () => root.render(ui));
  return {
    els, status,
    rerender: async (next: typeof first) => { current = next; await act(async () => root.render(strict ? <StrictMode><Probe /></StrictMode> : <Probe />)); },
    unmount: () => act(async () => root.unmount()),
  };
}

describe('the effects of the React canvas', () => {
  it('give the driver the props, the callbacks and the host on the first render, and a canvas of its own', async () => {
    const d = fakeDriver();
    const onSelect = vi.fn();
    const p = await probe(d, { draws: draws(), events: { onSelect } });
    expect(d.driver.setProps).toHaveBeenCalledTimes(1);
    expect(d.driver.setProps.mock.calls[0]![0]).toMatchObject({ scene, tool: 'move', theme: 'dark', camera: { x: 0, y: 0, zoom: 1 } });
    expect(d.driver.events.onSelect).toBe(onSelect);
    expect(d.driver.events.onStatus).toBe(p.status);
    expect(d.driver.mount).toHaveBeenCalledWith(p.els.host, p.els.children[0]);
    expect(p.els.children).toHaveLength(1);
  });

  it('hand the driver the host\'s answer to what is under a point with the callbacks, as the Svelte copy does (T9)', async () => {
    const d = fakeDriver();
    const first = vi.fn(() => 'a');
    const p = await probe(d, { draws: draws(), events: { hitAt: first } });
    expect(d.driver.events.hitAt).toBe(first);
    const second = vi.fn(() => null);
    await p.rerender({ draws: draws(), events: { hitAt: second } });
    expect(d.driver.events.hitAt).toBe(second);
  });

  it('hand the driver the callbacks of every render, and the props only when one of them changed', async () => {
    const d = fakeDriver();
    const base = draws();
    const first = vi.fn();
    const p = await probe(d, { draws: base, events: { onSelect: first } });
    const second = vi.fn();
    await p.rerender({ draws: { ...base }, events: { onSelect: second } }); // new object, same values of every prop
    expect(d.driver.events.onSelect).toBe(second);
    expect(d.driver.setProps).toHaveBeenCalledTimes(1);
    await p.rerender({ draws: { ...base, selection: ['a'] }, events: { onSelect: second } });
    expect(d.driver.setProps).toHaveBeenCalledTimes(2);
    expect(d.driver.setProps.mock.calls[1]![0].selection).toEqual(['a']);
  });

  it('hand over a new camera object each time it is a new object, leaving the comparison by value to the driver', async () => {
    const d = fakeDriver();
    const base = draws();
    const p = await probe(d, { draws: base, events: {} });
    await p.rerender({ draws: { ...base, camera: { x: 0, y: 0, zoom: 1 } }, events: {} }); // equal by value, a new object
    expect(d.driver.setProps).toHaveBeenCalledTimes(2);
    expect(d.driver.setProps.mock.calls[1]![0].camera).toEqual({ x: 0, y: 0, zoom: 1 });
    expect(d.driver.setProps.mock.calls[1]![0].camera).not.toBe(d.driver.setProps.mock.calls[0]![0].camera);
  });

  it('stop the driver and take the canvas out on unmount, and mount once however often it renders', async () => {
    const d = fakeDriver();
    const base = draws();
    const p = await probe(d, { draws: base, events: {} });
    await p.rerender({ draws: { ...base, tool: 'rect' }, events: {} });
    expect(d.driver.mount).toHaveBeenCalledTimes(1);
    await p.unmount();
    expect(d.log).toEqual(['mount', 'stop']);
    expect(p.els.children).toHaveLength(0);
  });

  it('in strict mode run twice and end with one canvas and one live mount', async () => {
    const d = fakeDriver();
    const p = await probe(d, { draws: draws(), events: {} }, true);
    expect(d.log).toEqual(['mount', 'stop', 'mount']);
    expect(p.els.children).toHaveLength(1);
    await p.unmount();
    expect(d.log).toEqual(['mount', 'stop', 'mount', 'stop']);
    expect(p.els.children).toHaveLength(0);
  });
});
