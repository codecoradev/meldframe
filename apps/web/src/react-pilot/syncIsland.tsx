// An island whose draws are not batched (docs/react-web-plan.md, section 7, slice S9). The island helper of web-chrome draws with
// `root.render`, which React schedules for a render of its own: when two snapshots reach the page before React gets to it, the
// props of the first are never drawn. That does not matter for a panel that draws the state, but the manage view reads the events
// of each snapshot (a refusal to say, the thing that was added, a rename that was made), and a snapshot whose events were never
// drawn loses them. Drawing each update inside `flushSync` makes React draw every snapshot's props, as Svelte flushes its own
// effects once per update. The update runs in a Svelte microtask, never inside a React render or effect, so `flushSync` is allowed.
import { createIsland } from '@meldframe/web-chrome/react/island';
import type { RootLike } from '@meldframe/web-chrome/react/island';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';

/** A root whose `render` is finished when it returns: `flush` runs the draw to its end (React's `flushSync`). */
export function syncRoot(root: RootLike, flush: (fn: () => void) => void): RootLike {
  return { render: (node) => flush(() => root.render(node)), unmount: () => root.unmount() };
}

export const syncIsland = createIsland({ createRoot: (el) => syncRoot(createRoot(el), flushSync), defer: (fn) => queueMicrotask(fn) });
