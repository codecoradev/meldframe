import { mount, unmount } from 'svelte';
import './app.css';
import { Engine, loadWasm } from '@meldframe/web-link/engine';
import { Shell, browserEnv, type CaptureRequest } from '@meldframe/web-link/shell';
import App from './App.svelte';
import Editor from './Editor.svelte';
import Fatal from './Fatal.svelte';
import { loadFile, reloadOnDivergence } from '@meldframe/web-document/file';
import { applyTheme, loadTheme } from '@meldframe/web-tokens/theme';

applyTheme(loadTheme());
const root = document.getElementById('app')!;
// From the environment of `just web` (vite.config.ts): the file to open and the actor of this client's ops (the dev token's). Sign-in replaces them in M2.
const [file, actor] = [__FILE__, __ACTOR__];

let shell: Shell | undefined;
let editor: ReturnType<typeof mount> | undefined;

async function start() {
  const page = new URLSearchParams(location.search);
  if (page.has('tokens')) return mount(App, { target: root });
  if (page.has('states')) return mount((await import('@meldframe/web-chrome/StateMatrix.svelte')).default, { target: root });
  if (page.has('layout')) return mount((await import('@meldframe/web-chrome/LayoutDemo.svelte')).default, { target: root }); // /?states and /?layout load on demand: the editor's bundle does not carry them
  await loadWasm();
  const { seq, nodes, variables, attribution } = await loadFile(fetch, file);
  // A fresh nonce per page load: the app never reuses an idempotency key (see `App::new`).
  const session = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
  const engine = Engine.create({ actor, session, nodes, seq, variables });
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
      engine, file, actor, attribution,
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
  mount(Fatal, { target: root, props: { message: e instanceof Error ? e.message : String(e) } });
});
