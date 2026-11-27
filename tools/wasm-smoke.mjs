// Runs the built wasm bridge under Node and checks the JSON boundary (the same cases as
// crates/app/meldframe-wasm/tests/bridge.rs, through the real wasm-bindgen glue).
// Usage: node tools/wasm-smoke.mjs <dir with the nodejs-target glue>
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const dir = resolve(process.argv[2] ?? '');
const { WasmApp, displayText } = createRequire(import.meta.url)(`${dir}/meldframe_wasm.js`);

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

// The one text helper of rule U9 for a screen with no app state (the page that says the file did not open).
assert.equal(displayText('a\u202eb\u200b\u00a0\u00a0c', 200), 'ab c');
assert.equal(displayText('x'.repeat(300), 200), 'x'.repeat(200));

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
assert.deepEqual([activity.lanes[0].view.kind, activity.lanes[0].view.name, activity.lanes[0].view.tone], ['agent', 'x', 'agent2'], 'the lane carries the actor view');
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
// A header with a field the app does not know is read as the server sends it (finding T6), and a review state that is not one is thrown as a string.
run({ type: 'frame', nowMs: T, frame: { type: 'batch', header: header(2, 1, 'b2', { extra: 1 }), unreviewedCount: 1 } });
assert.throws(() => run({ type: 'setBatchReview', batch: { actor: 'a', keyPrefix: 'k' }, state: 'approved' }), (e) => typeof e === 'string' && e.startsWith('action:'));
// A raw answer as text (finding T6): the app layer reads it, and a body that is not the server's is refused as unavailable.
const rawPage = { batches: [header(3, 1, 'b3')], hasMore: false, unreviewedCount: 4, canReview: true };
run({ type: 'wire', wire: { kind: 'batches', status: 200, body: JSON.stringify(rawPage) } });
assert.ok(JSON.parse(app.stateJson()).activity.rows.some((r) => r.n === 3), 'the row of the raw page is in the panel');
app.takeEventsJson();
run({ type: 'wire', wire: { kind: 'batches', status: 200, body: '<html>' } });
assert.deepEqual(JSON.parse(app.takeEventsJson()), [{ type: 'refused', code: 'unavailable' }]);

// A subscribe the server refused (finding T14): the app layer reads the text of the `error` frame and decides; the shell keeps no count.
const sub = new WasmApp('usr_me', 'web', 10, '[]', 0);
const runSub = (action) => JSON.parse(sub.dispatch(JSON.stringify(action)));
runSub({ type: 'start', nowMs: T });
runSub({ type: 'connected', nowMs: T });
assert.deepEqual(runSub({ type: 'frame', nowMs: T, frame: { type: 'resync', afterSeq: 0, latestSeq: 3 } }).filter((e) => e.type === 'subscribe'), [{ type: 'subscribe', afterSeq: 0 }]);
const refusal = (code, extra = {}) => ({ type: 'wire', wire: { kind: 'subscribeRefused', nowMs: T, text: JSON.stringify({ type: 'error', id: 's1', message: 'm', retryable: true, code, ...extra }) } });
assert.deepEqual(runSub(refusal('rate_limited', { retryAfterMs: 2000 })), [{ type: 'wakeAfter', afterMs: 2000 }]);
assert.deepEqual(runSub({ type: 'tick', nowMs: T + 2000 }), [{ type: 'subscribe', afterSeq: 0 }], 'sent again after the wait the server named');
assert.deepEqual(runSub(refusal('bad_request')), [], 'a subscribe before the marker waits for it');
assert.equal(JSON.parse(sub.stateJson()).connection.type, 'live');
runSub(refusal('forbidden'));
assert.equal(JSON.parse(sub.stateJson()).connection.type, 'offline', 'any other refusal gives the connection up');

// An op the server refused (finding T14): the shell hands over the status and the text of the body; the app reads the code and the flag, and a body that is not the server's is retried by its own rule.
const opApp = new WasmApp('usr_me', 'web', 11, '[]', 0);
const runOp = (action) => JSON.parse(opApp.dispatch(JSON.stringify(action)));
runOp({ type: 'start', nowMs: T });
runOp({ type: 'connected', nowMs: T });
const opKey = () => runOp({ type: 'createNode', kind: 'rectangle', x: 1, y: 2, parent: null }).find((e) => e.type === 'sendOps').ops[0].key;
const k1 = opKey();
assert.deepEqual(runOp({ type: 'wire', wire: { kind: 'opRefused', nowMs: T, key: k1, status: 502, body: '<html>bad gateway</html>' } }), [{ type: 'wakeAfter', afterMs: 1000 }], 'no error body: unavailable, and retryable');
assert.ok(JSON.parse(opApp.takeEventsJson()).some((e) => e.type === 'opRetry' && e.key === k1));
assert.equal(JSON.parse(opApp.nodesJson()).length, 1, 'the view keeps the node while the op waits');
runOp({ type: 'wire', wire: { kind: 'opRefused', nowMs: T + 1, key: k1, status: 400, body: JSON.stringify({ error: { code: 'E_TOO_LONG', message: 'm', retryable: false } }) } });
assert.ok(JSON.parse(opApp.takeEventsJson()).some((e) => e.type === 'opRejected' && e.code === 'E_TOO_LONG' && e.retryable === false), 'a refusal for good keeps the code the server wrote');
assert.equal(JSON.parse(opApp.nodesJson()).length, 0, 'and rolls the node back');
assert.deepEqual(runOp({ type: 'wire', wire: { kind: 'opRefused', nowMs: T, key: opKey(), status: 200, body: '{"accepted":[]}' } }), [], 'a 200 is nothing');

// The Export dialog (D9): the HTML of the selected frame is built in the state while the dialog is open.
const exp = new WasmApp('usr_me', 'web', 8, JSON.stringify([{ id: 'f', type: 'frame', pos: 'a0', name: 'Landing', w: 300, h: 200 }]), 0);
const runExp = (action) => JSON.parse(exp.dispatch(JSON.stringify(action)));
runExp({ type: 'select', ids: ['f'] });
assert.deepEqual(JSON.parse(exp.stateJson()).export.target, { id: 'f', name: 'Landing', fileStem: 'Landing' });
runExp({ type: 'openExport' });
const dialog = JSON.parse(exp.stateJson()).export.dialog;
assert.equal(dialog.format, 'tailwind');
assert.ok(dialog.html.includes('data-node="f"'), 'the document of the frame crosses the boundary');
runExp({ type: 'closeExport' });
assert.equal(JSON.parse(exp.stateJson()).export.dialog, null);

// The canvas flag (host configuration): the stored boxes until it is on, then the laid-out ones, through the real wasm.
const lay = new WasmApp('usr_me', 'web', 9, JSON.stringify([
  { id: 'f', type: 'frame', pos: 'a0', w: 200, h: 300, layout: 'column', padding: 10, gap: 5 },
  { id: 'b', type: 'rect', pos: 'a0', parentId: 'f', w: 1, h: 30, sw: 'fill' },
]), 0);
const boxB = () => JSON.parse(lay.sceneJson()).find((b) => b.id === 'b');
assert.deepEqual([boxB().x, boxB().w], [0, 1]);
assert.deepEqual(JSON.parse(lay.setLayoutOnCanvas(true)), [{ type: 'render', nodes: [] }]);
assert.deepEqual([boxB().x, boxB().y, boxB().w, boxB().h], [10, 10, 180, 30]);
assert.deepEqual(JSON.parse(lay.setLayoutOnCanvas(true)), [], 'the same value asks for nothing');
assert.equal(lay.takeEventsJson(), '[]', 'the flag raises no event');
assert.deepEqual(JSON.parse(lay.dropPreviewJson('["b"]', 30, 5, 0, 0)), { slot: { frame: 'f', index: 0, line: { x: 10, y: 10, w: 180, h: 0 } }, follow: [] }, 'the preview of a drop: the slot, as data, and nothing follows the pointer');
assert.deepEqual(JSON.parse(lay.dropPreviewJson('["b"]', 900, 900, 0, 0)), { slot: null, follow: ['b'] }, 'a drop out of the flow has no slot and the node follows the pointer');
assert.equal(lay.dropPreviewJson('b', 900, 900, 0, 0), 'null', 'ids that are not a JSON array have no preview');
lay.setLayoutOnCanvas(false);
assert.deepEqual([boxB().x, boxB().w], [0, 1]);
assert.equal(lay.dropPreviewJson('["b"]', 30, 5, 0, 0), 'null', 'no preview with the flag off');

// The text measurer (host configuration): a JavaScript function `(text, fontSize, fontWeight, wrapWidth | null) => [w, h]`
// sizes the hug texts of the laid-out scene; whatever else it does, the stub answers, so the scene never holds a NaN.
const meas = new WasmApp('usr_me', 'web', 10, JSON.stringify([
  { id: 'f', type: 'frame', pos: 'a0', w: 400, h: 100, layout: 'row', padding: 0, gap: 0 },
  { id: 't', type: 'text', pos: 'a0', parentId: 'f', text: 'Hello', fontSize: 20, fontWeight: 700, sw: 'hug', sh: 'hug' },
]), 0);
const boxT = () => JSON.parse(meas.sceneJson()).find((b) => b.id === 't');
const size = () => [boxT().w, boxT().h];
const calls = [];
let answer = () => [77, 31];
const measurer = (...args) => { calls.push(args); return answer(...args); };
assert.deepEqual(JSON.parse(meas.setMeasurer(measurer)), [], 'the flag is off: nothing to redraw, the measurer is not asked');
assert.equal(calls.length, 0, 'the stored scene asks nobody');
assert.deepEqual(JSON.parse(meas.setLayoutOnCanvas(true)), [{ type: 'render', nodes: [] }]);
assert.deepEqual(size(), [77, 31], 'the host sizes the hug text');
assert.deepEqual(calls[0], ['Hello', 20, 700, null], 'text, size, weight, and null for one line');
assert.deepEqual(JSON.parse(meas.setMeasurer(measurer)), [{ type: 'render', nodes: [] }], 'the flag is on: a redraw of everything');
answer = (_t, _s, _w, wrap) => (wrap === null ? [60, 24] : [40, 48]);
meas.setMeasurer(measurer);
assert.deepEqual(size(), [60, 24]);
const stub = [50, 24]; // the stub: half an em per character, ceil(1.2 em) high
for (const bad of [
  () => { throw new Error('no fonts'); },
  () => [NaN, 10], () => [10, NaN], () => [Infinity, 10], () => [-1, 10], () => [10, -1], () => [1e9, 10],
  () => [10], () => 5, () => null, () => undefined, () => ({ 0: 1, 1: 2 }), () => ['10', '10'], () => [10n, 10n],
]) {
  answer = bad;
  meas.setMeasurer(measurer);
  assert.deepEqual(size(), stub, String(bad));
}
assert.ok(JSON.parse(meas.sceneJson()).every((b) => [b.x, b.y, b.w, b.h].every(Number.isFinite)), 'no NaN in the scene');
assert.equal(meas.takeEventsJson(), '[]', 'the measurer raises no event');
for (const notAFunction of ['not a function', undefined, null, {}]) {
  assert.throws(() => meas.setMeasurer(notAFunction), (e) => e === 'measurer: not a function', 'refused at the boundary, as a string');
}

// Checkpoints (E5.3, ADR-0023): the list, a post, and a restore that waits for its row, through the real wasm. The file
// at a checkpoint comes from the server, so a restore needs no record of the app's own.
const ck = new WasmApp('usr_me', 'web', 11, '[]', 0);
const runCk = (action) => JSON.parse(ck.dispatch(JSON.stringify(action)));
runCk({ type: 'start', nowMs: T });
assert.ok(runCk({ type: 'connected', nowMs: T + 1 }).some((e) => e.type === 'fetchCheckpoints'), 'a subscribe reads the list');
const cpRow = (n, seq, kind, extra = {}) => ({ n, seq, kind, by: 'usr_me', atMs: T, ...extra });
runCk({ type: 'checkpointsLoaded', canCreate: true, checkpoints: [cpRow(1, 0, 'manual', { label: 'empty' }), cpRow(2, 0, 'runStart', { by: undefined, about: 'agent:x' })] });
let cps = JSON.parse(ck.stateJson());
assert.deepEqual(cps.checkpoints.map((c) => c.id), ['2', '1'], 'newest first, the text of n as the id');
assert.equal(cps.canCreateCheckpoint, true);
assert.equal(cps.checkpointsStatus, 'loaded');
const posted = runCk({ type: 'createCheckpoint', label: ' kept ' }).find((e) => e.type === 'postCheckpoint');
assert.deepEqual([posted.kind, posted.label, posted.about, posted.seq], ['manual', 'kept', null, 0]);
assert.equal(JSON.parse(ck.stateJson()).checkpoints.length, 2, 'the list changes when the server says');
runCk({ type: 'checkpointPosted', key: posted.key, checkpoint: cpRow(3, 0, 'manual', { label: 'kept' }) });
assert.equal(JSON.parse(ck.stateJson()).checkpoints[0].label, 'kept');
runCk({ type: 'createNode', kind: 'rectangle', x: 1, y: 2, parent: null });
assert.deepEqual(runCk({ type: 'restoreCheckpoint', id: '1' }), [{ type: 'fetchCheckpointState', n: 1 }]);
const before = runCk({ type: 'checkpointStateLoaded', n: 1, seq: 0, nodes: [] }).find((e) => e.type === 'postCheckpoint');
assert.deepEqual([before.kind, before.about], ['beforeRestore', null]);
const restored = runCk({ type: 'checkpointPosted', key: before.key, checkpoint: cpRow(4, 0, 'beforeRestore') });
assert.ok(restored.some((e) => e.type === 'sendOps' && e.ops.some((o) => o.command.type === 'removeNode')), 'the restore is sent once its row is made');
assert.equal(JSON.parse(ck.nodesJson()).length, 0, 'and the file is as it was at the checkpoint');
ck.takeEventsJson();
runCk({ type: 'checkpointRequestFailed', request: { type: 'list' }, code: 'forbidden' });
assert.deepEqual(JSON.parse(ck.takeEventsJson()), [{ type: 'refused', code: 'forbidden' }], 'a failed read is a refusal');
assert.throws(() => runCk({ type: 'checkpointsLoaded', canCreate: true, checkpoints: [{ n: 1, seq: 0, kind: 'snapshot', atMs: 1 }] }), (e) => typeof e === 'string' && e.startsWith('action:'));
assert.throws(() => runCk({ type: 'frame', nowMs: T, frame: { type: 'checkpoint', checkpoint: { n: 1 } } }), (e) => typeof e === 'string' && e.startsWith('action:'));

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
