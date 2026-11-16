import { mount } from 'svelte';
import type { Node } from '@meldframe/protocol';
import App from './App.svelte';
import { Engine, loadWasm } from './engine';
import { describeSnapshotFailure, shouldReload, showFatal } from './fatal';
import { Shell, browserEnv } from './shell';
import { applyTokens } from './tokens';

applyTokens();
const root = document.getElementById('app')!;
const file = __FILE__;
const actor = __ACTOR__;

/** The file as it is now: nodes and the sequence number to follow the live stream from. */
async function snapshot(): Promise<{ seq: number; nodes: Node[] }> {
  const res = await fetch(`/v1/files/${encodeURIComponent(file)}`);
  if (!res.ok) throw new Error(describeSnapshotFailure(res.status, await res.text().catch(() => '')));
  return res.json();
}

try {
  await loadWasm();
  const { seq, nodes } = await snapshot();
  // A fresh nonce per page load: the app never reuses an idempotency key (see `App::new`).
  const session = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
  const engine = Engine.create({ actor, session, nodes, seq });
  // When the file and the events no longer fit, load the snapshot again with a new App (a few times at most;
  // after that the page shows the reload button).
  engine.subscribe((s) => {
    if (s.events.some((e) => e.type === 'diverged') && shouldReload(sessionStorage, Date.now())) location.reload();
  });
  const shell = new Shell(engine, browserEnv(), { file });
  mount(App, { target: root, props: { engine, actor } });
  shell.start();
} catch (e) {
  showFatal(root, e instanceof Error ? e.message : String(e));
}
