import { mount } from 'svelte';
import type { Node } from '@meldframe/protocol';
import App from './App.svelte';
import { Engine, loadWasm } from './engine';
import { Shell, browserEnv } from './shell';
import { applyTokens } from './tokens';

applyTokens();
await loadWasm();
const file = __FILE__;
const actor = __ACTOR__;

/** The file as it is now: nodes and the sequence number to follow the live stream from. */
async function snapshot(): Promise<{ seq: number; nodes: Node[] }> {
  const res = await fetch(`/v1/files/${encodeURIComponent(file)}`);
  if (!res.ok) throw new Error(`snapshot: ${res.status} ${await res.text()}`);
  return res.json();
}

const { seq, nodes } = await snapshot();
// A fresh nonce per page load: the app never reuses an idempotency key (see `App::new`).
const session = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
const engine = Engine.create({ actor, session, nodes, seq });
// When the file and the events no longer fit, load the snapshot again with a new App.
engine.subscribe((s) => s.events.some((e) => e.type === 'diverged') && location.reload());
const shell = new Shell(engine, browserEnv(), { file });

mount(App, { target: document.getElementById('app')!, props: { engine, actor } });
shell.start();
