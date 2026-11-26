import { mount, unmount } from 'svelte';
import './app.css';
import { Engine, displayText, loadWasm } from '@meldframe/web-link/engine';
import { Shell, browserEnv, type CaptureRequest } from '@meldframe/web-transport/shell';
import App from './App.svelte';
import Editor from './Editor.svelte';
import ErrorPage from '@meldframe/web-feedback/ErrorPage.svelte';
import { pageFor } from '@meldframe/web-feedback/failure';
import { installPixiMeasurer } from './canvas/pixitext';
import { createLayoutSwitch, layoutChoice } from '@meldframe/web-link/layoutswitch';
import { loadFile, reloadOnDivergence } from '@meldframe/web-document/file';
import { applyTheme, loadTheme } from '@meldframe/web-tokens/theme';
import { mountPilot, pilotPage } from './react-pilot/pilot';
import { mountReactErrorPage, reactFeedback } from './react-pilot/feedback';
import { shellParts } from './react-pilot/shell';

applyTheme(loadTheme());
const root = document.getElementById('app')!;
// From the environment of `just web` (vite.config.ts): the file to open and the actor of this client's ops (the dev token's). Sign-in replaces them in M2.
const [file, actor] = [__FILE__, __ACTOR__];

let shell: Shell | undefined;
// Whether the WASM module loaded: only then can the error page make what a server said safe to show (it shows none otherwise).
let wasmReady = false;
let editor: ReturnType<typeof mount> | undefined;

async function start() {
  const page = new URLSearchParams(location.search);
  const pilot = pilotPage(location.search); // the React pilot (E7.3, S1): /?states&react; removed with the last Svelte page
  if (pilot) return mountPilot(pilot, root);
  if (page.has('tokens')) return mount(App, { target: root });
  if (page.has('states')) return mount((await import('@meldframe/web-chrome/StateMatrix.svelte')).default, { target: root });
  if (page.has('layout')) return mount((await import('@meldframe/web-shell/LayoutDemo.svelte')).default, { target: root, props: shellParts }); // /?states and /?layout load on demand: the editor's bundle does not carry them
  await loadWasm();
  wasmReady = true;
  const { seq, nodes, variables, attribution } = await loadFile(fetch, file);
  // A fresh nonce per page load: the app never reuses an idempotency key (see `App::new`).
  const session = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
  const engine = Engine.create({ actor, session, nodes, seq, variables });
  // Who changed each node before this page loaded: the app layer takes it in (made safe like any actor) and the Layers panel draws it.
  engine.dispatch({ type: 'attribute', actors: Object.fromEntries(Object.entries(attribution).map(([id, a]) => [id, a.actor])) });
  // "Auto layout on canvas" (the palette entry; on by default): the canvas draws the laid-out boxes, with Pixi's text metrics on the
  // page's fonts. /?canvaslayout=0 and =1 force it for this page; the person's choice is remembered in this browser (AGENTS.md).
  const storage = () => { try { return localStorage; } catch { return null; } };
  const layout = createLayoutSwitch(engine, () => installPixiMeasurer(engine), layoutChoice(location.search, storage), storage);
  // When the file and the events no longer fit, start again from a fresh snapshot (a few times at most; after that
  // the page shows the reload button).
  engine.subscribe((s) => {
    if (reloadOnDivergence(s.events, () => sessionStorage, Date.now())) location.reload();
  });
  const captureHandlers = new Set<(r: CaptureRequest) => void>();
  shell = new Shell(engine, browserEnv(), {
    file,
    // With a handler the shell says this client can capture; the editor answers each request (capture.ts).
    onCapture: (r) => captureHandlers.forEach((h) => h(r)),
  });
  const sh = shell;
  editor = mount(Editor, { target: root, props: {
      engine, file, actor, layout,
      captures: {
        subscribe: (fn: (r: CaptureRequest) => void) => {
          captureHandlers.add(fn);
          return () => captureHandlers.delete(fn);
        },
        fail: (id: string, code: string, message: string) => sh.captureFailed(id, code, message),
      },
    },
  });
  shell.start();
}

start().catch((e: unknown) => {
  shell?.stop(); // a page that failed half way keeps no socket, timer or canvas
  if (editor) void unmount(editor);
  root.replaceChildren();
  // A trap inside the helper must not blank the page that reports a failure: it then shows what the page says itself.
  const safe = wasmReady ? (text: string, max: number) => { try { return displayText(text, max); } catch { return ''; } } : null;
  const page = pageFor(e, { dev: import.meta.env.DEV, safe });
  const again = () => location.reload();
  const svelte = () => mount(ErrorPage, { target: root, props: { page, onAction: again } });
  // /?react=feedback (E7.3, S3): the React copy loads on demand, and a chunk that does not load (the server or the network is
  // the very thing that is down) must not leave a blank page: the Svelte copy is in the entry and draws instead.
  if (reactFeedback(location.search)) {
    mountReactErrorPage(root, page, again).catch(() => {
      root.replaceChildren();
      svelte();
    });
  } else svelte();
});
