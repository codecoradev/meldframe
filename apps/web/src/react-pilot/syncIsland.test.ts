import { describe, expect, it } from 'vitest';
import { syncRoot } from './syncIsland';

// The root is a stub (the Node tests have no DOM): what is asked is that every `render` is run to its end before it returns, so that two
// draws in a row are two draws and not one, and that `unmount` is left alone.
describe('a root that draws each update to its end', () => {
  it('runs each render inside the flush, in order, and so draws the props of every update', () => {
    const calls: string[] = [];
    const root = { render: (n: unknown) => calls.push(`render ${String(n)}`), unmount: () => calls.push('unmount') };
    const flushed = syncRoot(root as never, (fn) => {
      calls.push('flush start');
      fn();
      calls.push('flush end');
    });
    flushed.render('a');
    flushed.render('b');
    flushed.unmount();
    expect(calls).toEqual(['flush start', 'render a', 'flush end', 'flush start', 'render b', 'flush end', 'unmount']);
  });
});
