import { mount, unmount } from 'svelte';
import './app.css';
import { Engine, loadWasm } from '@meldframe/web-link/engine';
import { Shell, browserEnv } from '@meldframe/web-link/shell';
import App from './App.svelte';
import Editor from './Editor.svelte';
import Fatal from './Fatal.svelte';
import { loadFile, reloadOnDivergence } from '@meldframe/web-document/file';
import { applyTheme, loadTheme } from './theme';

applyTheme(loadTheme());
const root = document.getElementById('app')!;
// `__FILE__` and `__ACTOR__` come from the environment of `just web` (vite.config.ts): the file to open, and the actor
// the server stamps on this client's ops (the dev token's actor). Real sign-in replaces them in M2.
const file = __FILE__;
const actor = __ACTOR__;

let shell: Shell | undefined;
let editor: ReturnType<typeof mount> | undefined;

async function start() {
  // The page of the design tokens stays at /?tokens.
  if (new URLSearchParams(location.search).has('tokens')) return mount(App, { target: root });
  await loadWasm();
  const { seq, nodes } = await loadFile(fetch, file);
  // A fresh nonce per page load: the app never reuses an idempotency key (see `App::new`).
  const session = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
  const engine = Engine.create({ actor, session, nodes, seq });
  // When the file and the events no longer fit, start again from a fresh snapshot (a few times at most; after that
  // the page shows the reload button).
  engine.subscribe((s) => {
    if (reloadOnDivergence(s.events, () => sessionStorage, Date.now())) location.reload();
  });
  shell = new Shell(engine, browserEnv(), { file });
  editor = mount(Editor, { target: root, props: { engine, file } });
  shell.start();
}

start().catch((e: unknown) => {
  shell?.stop(); // a page that failed half way keeps no socket, timer or canvas
  if (editor) void unmount(editor);
  root.replaceChildren();
  mount(Fatal, { target: root, props: { message: e instanceof Error ? e.message : String(e) } });
});
