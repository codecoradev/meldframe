// An island whose draws are not batched (docs/react-web-plan.md, section 7, slice S9). TEMPORARY: delete this file and its test when the
// shared island flush lands (S8, branch fix/web-island-flush: the island helper of web-chrome then draws through `flushSync` itself), and
// import `island` from the shared helper, as VariablesPanelSwitch.svelte does.
// The island helper drew with `root.render`, which React schedules for a render of its own, and a later update replaces an earlier one that
// was not drawn yet. The manage view reads the events of each snapshot (a refusal to say, the thing that was added, a rename that was
// made), so a snapshot whose props were never drawn loses its events. The cause is inferred, not shown by a test of this slice: with
// `?react=variables,manage` a check of manage-e2e.mjs ("a variable removed by another actor while its field is open") read an empty error
// line in 4 runs out of 4 and passed with the dialog alone, and a log of the body's effect showed that it never saw the events array that
// held the refusal; that fits two updates in one task and does not prove it. Drawing each update inside `flushSync` made the check pass.
// It does not help when the host itself hands the events of two frames in one update. The update runs in a Svelte microtask, so `flushSync`
// is not called inside a React render or effect (the one failure mode of `flushSync`; no test of this slice covers it).
import { createIsland } from '@meldframe/web-chrome/react/island';
import type { RootLike } from '@meldframe/web-chrome/react/island';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';

/** A root whose `render` is finished when it returns: `flush` runs the draw to its end (React's `flushSync`). */
export function syncRoot(root: RootLike, flush: (fn: () => void) => void): RootLike {
  return { render: (node) => flush(() => root.render(node)), unmount: () => root.unmount() };
}

export const syncIsland = createIsland({ createRoot: (el) => syncRoot(createRoot(el), flushSync), defer: (fn) => queueMicrotask(fn) });
