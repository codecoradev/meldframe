import { mount } from 'svelte';
import App from './App.svelte';
import { Engine, loadWasm } from './engine';
import { applyTokens } from './tokens';

applyTokens();
await loadWasm();
// A fresh nonce per page load: the app never reuses an idempotency key (see `App::new`).
const session = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
const actor = new URLSearchParams(location.search).get('actor') ?? 'usr_web';
const engine = Engine.create({ actor, session, nodes: [], seq: 0 });

mount(App, { target: document.getElementById('app')!, props: { engine, actor } });
