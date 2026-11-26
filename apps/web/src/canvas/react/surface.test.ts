import { describe, expect, it, vi } from 'vitest';
import { mountOn } from './surface';

/** Just enough of an element tree: `prepend` puts a child first, `remove` takes it out. */
function fakes() {
  const children: { className: string; remove(): void }[] = [];
  const host = {
    prepend(el: (typeof children)[number]) {
      children.unshift(el);
    },
  };
  const doc = {
    createElement: vi.fn((tag: string) => {
      const el = { tag, className: '', remove: () => void children.splice(children.indexOf(el), 1) };
      return el;
    }),
  };
  const stops: ReturnType<typeof vi.fn>[] = [];
  const driver = {
    mount: vi.fn(() => {
      const stop = vi.fn();
      stops.push(stop);
      return stop;
    }),
  };
  return { children, host: host as unknown as HTMLElement, doc: doc as unknown as Document, driver, stops, created: doc.createElement };
}

describe('mountOn, the effect of the React canvas', () => {
  it("gives the driver a canvas of its own, in front of the host's other children, with the class the stylesheet knows", () => {
    const f = fakes();
    f.children.push({ className: 'canvas-failed', remove: () => {} });
    mountOn(f.driver, f.host, f.doc);
    expect(f.created).toHaveBeenCalledWith('canvas');
    expect(f.children.map((c) => c.className)).toEqual(['canvas-surface', 'canvas-failed']);
    expect(f.driver.mount).toHaveBeenCalledWith(f.host, f.children[0]);
  });

  it('the cleanup stops the driver, then takes the canvas out', () => {
    const f = fakes();
    const undo = mountOn(f.driver, f.host, f.doc);
    undo();
    expect(f.stops[0]).toHaveBeenCalledTimes(1);
    expect(f.children).toHaveLength(0);
  });

  it('run, undone and run again (React does that to an effect in development), it leaves one canvas, and the second is not the first', () => {
    const f = fakes();
    const undo = mountOn(f.driver, f.host, f.doc);
    const first = f.children[0];
    undo();
    const again = mountOn(f.driver, f.host, f.doc);
    expect(f.children).toHaveLength(1);
    expect(f.children[0]).not.toBe(first); // a new surface: Pixi never gets a canvas that an earlier application used
    expect(f.driver.mount).toHaveBeenCalledTimes(2);
    again();
    expect(f.children).toHaveLength(0);
  });
});
