// What the shell does for the checkpoints (E5.3, ADR-0023): it carries out the three effects over the list, create and state
// routes. The answers and the `checkpoint` frame are not read here: the shell hands the status and the text to the app layer
// (`Action.wire`), which parses and checks them (finding T6). A failed request names itself, as the app reads it.
import type { Action, Effect, Wire } from '@meldframe/protocol';

type CheckpointEffect = Extract<Effect, { type: 'fetchCheckpoints' | 'postCheckpoint' | 'fetchCheckpointState' }>;

const failed = (request: Extract<Action, { type: 'checkpointRequestFailed' }>['request']): Action => ({ type: 'checkpointRequestFailed', request, code: 'unavailable' });

/** One request of the checkpoint routes: how to send it, what its answer is called, and what a network failure is. */
export interface CheckpointCall {
  method: 'GET' | 'POST';
  path: string;
  body?: unknown;
  /** The raw answer, as the app layer reads it. */
  wire: (status: number, body: string) => Wire;
  failure: Action;
}

/** The request an effect stands for. A null label or `about` is left out of the body. */
export function checkpointCall(file: string, e: CheckpointEffect): CheckpointCall {
  const path = `/v1/files/${encodeURIComponent(file)}/checkpoints`;
  switch (e.type) {
    case 'fetchCheckpoints':
      return { method: 'GET', path, wire: (status, body) => ({ kind: 'checkpoints', status, body }), failure: failed({ type: 'list' }) };
    case 'postCheckpoint': {
      const body = { key: e.key, kind: e.kind, seq: e.seq, ...(e.label !== null ? { label: e.label } : {}), ...(e.about !== null ? { about: e.about } : {}) };
      return { method: 'POST', path, body, wire: (status, text) => ({ kind: 'checkpointPosted', key: e.key, status, body: text }), failure: failed({ type: 'post', key: e.key }) };
    }
    case 'fetchCheckpointState': {
      const n = Math.trunc(e.n);
      return { method: 'GET', path: `${path}/${n}/state`, wire: (status, body) => ({ kind: 'checkpointState', n, status, body }), failure: failed({ type: 'state', n }) };
    }
  }
}
