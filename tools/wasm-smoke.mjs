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
// The scene: the painted boxes of the same document, from the Rust render step (E3.1).
const scene = JSON.parse(app.sceneJson());
assert.equal(scene.length, 1);
assert.equal(scene[0].id, nodes[0].id);
assert.equal(scene[0].paint, 'rect');
assert.equal(scene[0].parent, null);
assert.deepEqual([scene[0].x, scene[0].y], [10, 20]);
const state = JSON.parse(app.stateJson());
assert.equal(state.selection.length, 1);
assert.equal(state.history.canUndo, true);
assert.ok(JSON.parse(app.takeEventsJson()).some((e) => e.type === 'documentChanged'));
assert.equal(app.takeEventsJson(), '[]', 'taking the events drains them');

// The Activity panel (E5.1): actions and frames in as JSON, effects and state out, through the real wasm.
const T = 1_700_000_000_000;
const header = (n, rev, prefix, extra = {}) => ({
  n, rev, actor: 'agent:x', keyPrefix: prefix, tool: 'apply_commands', commands: 1, written: 1, atomic: false,
  firstSeq: 1, lastSeq: 1, atMs: T, review: 'unreviewed', ...extra,
});
assert.deepEqual(state.activity.rows, [], 'a fresh panel is empty');
assert.deepEqual(run({ type: 'openActivity' }), [{ type: 'fetchBatches', beforeN: null, afterN: null, limit: 100 }]);
run({ type: 'frame', nowMs: T, frame: { type: 'batch', header: header(1, 1, 'b1', { note: 'Hero\u202e' }), unreviewedCount: 1 } });
let activity = JSON.parse(app.stateJson()).activity;
assert.equal(activity.rows[0].n, 1);
assert.equal(activity.rows[0].note, 'Hero', 'the note is checked again: the bidi override is stripped');
assert.equal(activity.unreviewedCount, 1);
assert.equal(activity.lanes[0].actor, 'agent:x');
assert.ok(JSON.parse(app.takeEventsJson()).some((e) => e.type === 'activityChanged'));
run({ type: 'batchesLoaded', page: { batches: [header(1, 1, 'b1')], hasMore: false, unreviewedCount: 1, canReview: true } });
assert.deepEqual(
  run({ type: 'setBatchReview', batch: { actor: 'agent:x', keyPrefix: 'b1' }, state: 'accepted' }),
  [{ type: 'putBatchReview', n: 1, state: 'accepted' }],
);
run({ type: 'batchReviewed', batch: header(1, 2, 'b1', { review: 'accepted', reviewedBy: 'usr_me', reviewedAtMs: T + 1 }) });
activity = JSON.parse(app.stateJson()).activity;
assert.equal(activity.rows[0].review, 'accepted');
assert.equal(activity.rows[0].reviewedBy, 'usr_me');
run({ type: 'frame', nowMs: T, frame: { type: 'touch', actor: 'agent:x', ids: ['a'], seq: 2 } });
const ring = JSON.parse(app.stateJson()).activity.touches[0];
assert.deepEqual([ring.actor, ring.ids, ring.persistent], ['agent:x', ['a'], false]);
app.takeEventsJson();
run({ type: 'batchRequestFailed', code: 'forbidden' });
assert.deepEqual(JSON.parse(app.takeEventsJson()), [{ type: 'refused', code: 'forbidden' }], 'a failed request is a refusal');
// A header with a field the app does not know, and a review state that is not one, are thrown as strings.
assert.throws(() => run({ type: 'frame', nowMs: T, frame: { type: 'batch', header: header(2, 1, 'b2', { extra: 1 }), unreviewedCount: 1 } }), (e) => typeof e === 'string' && e.startsWith('action:'));
assert.throws(() => run({ type: 'setBatchReview', batch: { actor: 'a', keyPrefix: 'k' }, state: 'approved' }), (e) => typeof e === 'string' && e.startsWith('action:'));

// The Export dialog (D9): the HTML of the selected frame is built in the state while the dialog is open.
const exp = new WasmApp('usr_me', 'web', 8, JSON.stringify([{ id: 'f', type: 'frame', pos: 'a0', name: 'Landing', w: 300, h: 200 }]), 0);
const runExp = (action) => JSON.parse(exp.dispatch(JSON.stringify(action)));
runExp({ type: 'select', ids: ['f'] });
assert.deepEqual(JSON.parse(exp.stateJson()).export.target, { id: 'f', name: 'Landing' });
runExp({ type: 'openExport' });
const dialog = JSON.parse(exp.stateJson()).export.dialog;
assert.equal(dialog.format, 'tailwind');
assert.ok(dialog.html.includes('data-node="f"'), 'the document of the frame crosses the boundary');
runExp({ type: 'closeExport' });
assert.equal(JSON.parse(exp.stateJson()).export.dialog, null);

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
