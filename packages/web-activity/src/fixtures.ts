// Made-up state for the tests: no server, no wasm. A row and a lane with every field the app layer sends.
import type { ActivityLane, ActivityState, BatchRow } from '@meldframe/protocol';
import { actorView } from '@meldframe/web-document/fixtures';

export const row = (over: Partial<BatchRow> = {}): BatchRow => ({
  actor: 'agent:claude', keyPrefix: 'k1', n: 1, rev: 1, firstSeq: 1, lastSeq: 3, commands: 3, written: 3, atomic: true, atMs: 0,
  note: null, status: 'applied', review: 'unreviewed', reviewedBy: null, reviewedAtMs: null,
  summary: { added: 0, changed: 1, removed: 0, names: ['Hero'] }, canShow: true, revertable: true, inEffect: null, ...over,
});

export const lane = (over: Partial<ActivityLane> = {}): ActivityLane => ({
  actor: 'agent:claude', view: actorView(over.actor ?? 'agent:claude'), state: 'idle', ops: 3, lastAtMs: 0, runs: [], batches: ['k1'], ...over,
});

export const activity = (over: Partial<ActivityState> = {}): ActivityState => ({
  unreviewedCount: 0, canReview: false, lanes: [], rows: [], selected: null, hasOlder: false, touches: [], ...over,
});
