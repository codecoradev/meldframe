// Tests of the React pipeline (slice S0): the TSX transform, `useAppState`, and the island helper. They run in
// Node with no DOM library. Server rendering covers the markup. For effects (subscriptions) a real React root is
// mounted on a stand-in container with just enough of a DOM for React to start; the probe components return null,
// so nothing is ever written to it. What a browser alone can prove (real markup, focus) is left to the Playwright scripts.
import { type ReactNode, act } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { TestMode, TestSample } from './TestSample';
import { createIsland, island } from './island';
import { type StateSource, useAppState } from './useAppState';

interface Snap {
  mode: string;
  n: number;
}

/** An engine like `Engine`: the same snapshot object until something changes, a set of listeners. */
function fakeEngine(first: Snap) {
  let snap = first;
  const listeners = new Set<(s: Snap) => void>();
  const stats = { subscribes: 0 };
  const source: StateSource<Snap> = {
    snapshot: () => snap,
    subscribe(fn) {
      stats.subscribes++;
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
  };
  return {
    source,
    stats,
    listeners,
    set(next: Snap) {
      snap = next;
      for (const fn of [...listeners]) fn(snap);
    },
  };
}

const view = { document: { activeElement: null, body: null }, HTMLIFrameElement: class {} };
const container = () =>
  ({
    nodeType: 1,
    nodeName: 'DIV',
    tagName: 'DIV',
    namespaceURI: 'http://www.w3.org/1999/xhtml',
    addEventListener() {},
    removeEventListener() {},
    ownerDocument: { defaultView: view, addEventListener() {}, removeEventListener() {} },
  }) as unknown as HTMLElement;

beforeAll(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true, window: view });
});
afterAll(() => {
  Reflect.deleteProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT');
  Reflect.deleteProperty(globalThis, 'window');
});
afterEach(() => vi.restoreAllMocks());

describe('the TSX pipeline', () => {
  it('compiles JSX and renders it on the server', () => {
    expect(renderToStaticMarkup(<TestSample label="Hello" />)).toBe('<p data-testid="sample">Hello</p>');
  });
});

describe('useAppState', () => {
  it('renders the selected part on the server, and the whole snapshot without a selector', () => {
    const e = fakeEngine({ mode: 'edit', n: 1 });
    const engine = { snapshot: () => ({ state: { mode: 'edit' } }), subscribe: () => () => {} };
    expect(renderToStaticMarkup(<TestMode engine={engine} />)).toBe('<p data-testid="mode">edit</p>');
    const Whole = () => <p>{JSON.stringify(useAppState(e.source))}</p>;
    expect(renderToStaticMarkup(<Whole />)).toBe('<p>{&quot;mode&quot;:&quot;edit&quot;,&quot;n&quot;:1}</p>');
  });

  async function mount(e: ReturnType<typeof fakeEngine>, pick: (s: Snap) => unknown, isEqual?: (a: unknown, b: unknown) => boolean) {
    const seen: unknown[] = [];
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
    const Reader = ({ source, select }: { source: StateSource<Snap>; select: (s: Snap) => unknown }) => {
      seen.push(useAppState(source, select, isEqual));
      return null;
    };
    const root = createRoot(container());
    const draw = (source: StateSource<Snap> = e.source, select = pick): Promise<void> => act(async () => root.render(<Reader source={source} select={select} />));
    await draw();
    return { seen, errors, root, draw };
  }

  it('subscribes once and draws again only when the selected part changed', async () => {
    const e = fakeEngine({ mode: 'edit', n: 1 });
    const { seen, errors, root } = await mount(e, (s) => s.mode);
    expect(e.listeners.size).toBe(1);
    expect(seen).toEqual(['edit']);
    await act(async () => e.set({ mode: 'edit', n: 2 })); // a new snapshot, the same mode
    expect(seen).toEqual(['edit']);
    await act(async () => e.set({ mode: 'view', n: 2 }));
    expect(seen).toEqual(['edit', 'view']);
    await act(async () => root.unmount());
    expect(e.listeners.size).toBe(0);
    expect(errors).not.toHaveBeenCalled();
  });

  it('does not subscribe again when the component is drawn again with a new inline selector', async () => {
    const e = fakeEngine({ mode: 'edit', n: 1 });
    const { seen, draw, errors } = await mount(e, (s) => s.n);
    await draw(e.source, (s) => s.n);
    await draw(e.source, (s) => s.n);
    expect(e.stats.subscribes).toBe(1);
    expect(e.listeners.size).toBe(1);
    await act(async () => e.set({ mode: 'edit', n: 5 }));
    expect(seen.at(-1)).toBe(5);
    expect(errors).not.toHaveBeenCalled();
  });

  it('subscribes again only when the engine is another one, and lets go of the old one', async () => {
    const a = fakeEngine({ mode: 'a', n: 0 });
    const b = fakeEngine({ mode: 'b', n: 0 });
    const { seen, draw } = await mount(a, (s) => s.mode);
    await draw(b.source);
    expect(a.listeners.size).toBe(0);
    expect(b.listeners.size).toBe(1);
    expect(a.stats.subscribes).toBe(1);
    expect(b.stats.subscribes).toBe(1);
    expect(seen.at(-1)).toBe('b');
  });

  it('keeps the earlier value when isEqual says a derived value did not change, and never warns about caching', async () => {
    const e = fakeEngine({ mode: 'edit', n: 1 });
    const same = (a: unknown, b: unknown) => (a as { big: boolean }).big === (b as { big: boolean }).big;
    const { seen, errors } = await mount(e, (s) => ({ big: s.n > 5 }), same);
    const first = seen[0];
    await act(async () => e.set({ mode: 'edit', n: 2 }));
    expect(seen).toEqual([first]);
    await act(async () => e.set({ mode: 'edit', n: 9 }));
    expect(seen).toHaveLength(2);
    expect(seen[1]).toEqual({ big: true });
    expect(errors).not.toHaveBeenCalled();
  });

  it('survives a selector that builds a new object per call, with one draw per snapshot (no loop, no warning)', async () => {
    const e = fakeEngine({ mode: 'edit', n: 1 });
    const { seen, errors } = await mount(e, (s) => ({ n: s.n }));
    expect(seen).toHaveLength(1);
    await act(async () => e.set({ mode: 'edit', n: 2 }));
    expect(seen).toHaveLength(2);
    expect(errors).not.toHaveBeenCalled();
  });
});

describe('the island action', () => {
  const draws = (log: string[], name = 'p') => ({ n }: { n: number }): ReactNode => {
    log.push(`${name}${n}`);
    return null;
  };

  function fakeRoots() {
    const deferred: Array<() => void> = [];
    const calls: string[] = [];
    let made = 0;
    const island = createIsland({
      createRoot: () => {
        const id = ++made;
        return { render: () => calls.push(`render${id}`), unmount: () => calls.push(`unmount${id}`) };
      },
      defer: (fn) => deferred.push(fn),
    });
    return { island, calls, flush: () => deferred.splice(0).forEach((f) => f()) };
  }
  const el = () => ({}) as HTMLElement;

  it('draws on use, draws again on update with the same root, and unmounts after the stack on destroy', () => {
    const r = fakeRoots();
    const a = r.island(el(), { component: draws([]), props: { n: 1 } });
    a.update({ component: draws([]), props: { n: 2 } });
    expect(r.calls).toEqual(['render1', 'render1']);
    a.destroy();
    expect(r.calls).toEqual(['render1', 'render1']); // not inside the current stack
    r.flush();
    expect(r.calls).toEqual(['render1', 'render1', 'unmount1']);
  });

  it('gives each element a root of its own', () => {
    const r = fakeRoots();
    const a = r.island(el(), { component: draws([]), props: { n: 1 } });
    const b = r.island(el(), { component: draws([]), props: { n: 1 } });
    b.update({ component: draws([]), props: { n: 2 } });
    a.destroy();
    r.flush();
    expect(r.calls).toEqual(['render1', 'render2', 'render2', 'unmount1']);
  });

  it('unmounts once, however often it is destroyed, and drops a draw that follows', () => {
    const r = fakeRoots();
    const a = r.island(el(), { component: draws([]), props: { n: 1 } });
    a.destroy();
    a.destroy();
    a.update({ component: draws([]), props: { n: 2 } });
    r.flush();
    expect(r.calls).toEqual(['render1', 'unmount1']);
  });

  it('runs on a real React root: new props reach the component and destroy runs its cleanups', async () => {
    const log: string[] = [];
    const Probe = ({ n }: { n: number }): ReactNode => {
      log.push(`draw${n}`);
      return null;
    };
    const node = container();
    let handle!: ReturnType<typeof island<{ n: number }>>;
    await act(async () => {
      handle = island(node, { component: Probe, props: { n: 1 } });
    });
    await act(async () => handle.update({ component: Probe, props: { n: 2 } }));
    expect(log).toEqual(['draw1', 'draw2']);
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
    await act(async () => {
      handle.destroy();
      await Promise.resolve();
    });
    await act(async () => handle.update({ component: Probe, props: { n: 3 } }));
    expect(log).toEqual(['draw1', 'draw2']);
    expect(errors).not.toHaveBeenCalled();
  });
});
