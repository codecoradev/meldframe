// The switch of the React pilot (docs/react-web-plan.md, section 7, slice S1). Temporary and reversible: the last slice
// deletes this folder and the one line in main.ts that calls it. Nothing here changes what a page shows by default.
//   /?states&react       the state matrix drawn by React (the primitives of web-chrome/src/react, Base UI under Tooltip and Menu)
//   /?overlays=svelte    development only: the Svelte IconButton, Menu and tooltip with fixed rows and test ids
//   /?overlays=react     development only: the same fixture drawn by the React copies
//   /?react=export       the editor with the Export section and dialog drawn by React (E7.3, S2). `reactFor` reads it;
//                        ExportSectionSwitch.svelte and ExportDialogSwitch.svelte, which Editor.svelte imports, act on it.
//                        Further panels join the list (/?react=export,activity); the last slice deletes the whole switch.
//   /?react=palette     the command palette and the shortcut sheet drawn by React, in the editor and on /?layout (E7.3, S4):
//                        the Layout draws what `shell.ts` hands it (PaletteSwitch.svelte, ShortcutHintSwitch.svelte).
// packages/web-chrome/scripts/overlays-e2e.mjs asks the two fixtures the same questions; states-e2e.mjs runs on the first with MELDFRAME_UI=react.
import { mount } from 'svelte';

export type PilotPage = 'states' | 'overlays-svelte' | 'overlays-react';

/** The pilot page a query string asks for, or null for every other page. The overlay fixtures exist in development only. */
export function pilotPage(search: string, dev: boolean = import.meta.env.DEV): PilotPage | null {
  const q = new URLSearchParams(search);
  if (q.has('states') && q.has('react')) return 'states';
  const overlays = q.get('overlays');
  if (dev && (overlays === 'svelte' || overlays === 'react')) return `overlays-${overlays}`;
  return null;
}

/** Whether this page asks for the React copy of `panel` (`/?react=export`, or `/?react=export,activity`); the default is Svelte. */
export function reactFor(panel: string, search: string): boolean {
  return (new URLSearchParams(search).get('react') ?? '').split(',').includes(panel);
}

/** Draws the page on `target`. The React side loads on demand: the editor's bundle does not carry it. */
export async function mountPilot(page: PilotPage, target: HTMLElement) {
  if (page === 'overlays-svelte' && import.meta.env.DEV) {
    return mount((await import('./OverlayFixtureSvelte.svelte')).default, { target });
  }
  const host = (await import('./PilotHost.svelte')).default;
  if (page === 'overlays-react' && import.meta.env.DEV) {
    return mount(host, { target, props: { component: (await import('./OverlayFixture')).OverlayFixture } });
  }
  return mount(host, { target, props: { component: (await import('@meldframe/web-chrome/react/StateMatrix')).StateMatrix } });
}
