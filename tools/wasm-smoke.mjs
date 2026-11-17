// Runs the built wasm bridge under Node and checks the JSON boundary (the same cases as
// crates/app/meldframe-wasm/tests/bridge.rs, through the real wasm-bindgen glue).
// Usage: node tools/wasm-smoke.mjs <dir with the nodejs-target glue>
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const dir = resolve(process.argv[2] ?? '');
const { WasmApp } = createRequire(import.meta.url)(`${dir}/meldframe_wasm.js`);

const app = new WasmApp('usr_me', 'web', 7, '[]', 0);
const run = (action) => JSON.parse(app.dispatch(JSON.stringify(action)));

assert.deepEqual(run({ type: 'start', nowMs: 1 }), [{ type: 'subscribe', afterSeq: 0 }]);
run({ type: 'connected', nowMs: 2 });
const fx = run({ type: 'createNode', kind: 'rectangle', x: 10, y: 20, parent: null });
assert.ok(fx.some((e) => e.type === 'sendOps'), 'creating a node sends an op');

const nodes = JSON.parse(app.nodesJson());
assert.equal(nodes.length, 1);
const state = JSON.parse(app.stateJson());
assert.equal(state.selection.length, 1);
assert.equal(state.history.canUndo, true);
assert.ok(JSON.parse(app.takeEventsJson()).some((e) => e.type === 'documentChanged'));
assert.equal(app.takeEventsJson(), '[]', 'taking the events drains them');

// Errors are thrown as strings, not as a trap.
assert.throws(() => app.dispatch('not json'), (e) => typeof e === 'string' && e.startsWith('action:'));
assert.throws(() => new WasmApp('usr_me', 'web', 1, '{', 0), (e) => typeof e === 'string' && e.startsWith('nodes:'));
for (const seq of [1.5, -1, NaN, Infinity, 2 ** 53]) {
  assert.throws(() => new WasmApp('usr_me', 'web', 1, '[]', seq), (e) => typeof e === 'string' && e.startsWith('seq:'), `seq ${seq}`);
}
// A session that would be coerced (-1 to 4294967295, NaN to 0) is refused instead.
for (const session of [-1, NaN, 2 ** 32, 0.5]) {
  assert.throws(() => new WasmApp('usr_me', 'web', session, '[]', 0), (e) => typeof e === 'string' && e.startsWith('session:'), `session ${session}`);
}
new WasmApp('usr_me', 'web', 2 ** 32 - 1, '[]', 2 ** 53 - 1).free(); // the largest values are accepted

console.log('wasm-smoke: ok');
