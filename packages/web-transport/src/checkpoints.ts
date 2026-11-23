// What the shell does for the checkpoints (E5.3, ADR-0023): it maps the `checkpoint` frame of the live socket to the app
// layer's frame, carries out the three effects over the list, create and state routes, and maps their answers to
// actions. Everything here comes from the network and is trusted for nothing: each field is checked for type and bound,
// only the fields the app layer knows are copied (it refuses the rest), and a code is capped before it reaches the app.
// Pure functions; the request itself goes through `Env`.
import type { Action, CheckpointKind, CheckpointRequest, CheckpointRow, Effect, Frame } from '@meldframe/protocol';
import { safeCode } from './batches';

const U32 = 2 ** 32 - 1;
const BIG = Number.MAX_SAFE_INTEGER;
const MAX_ACTOR = 256;
/** A label is 128 characters; a character is at most two UTF-16 units. The app checks it again. */
const MAX_LABEL = 2000;
/** A file keeps 100 rows at most. */
const MAX_ROWS = 100;
/** A file at a checkpoint holds the nodes of one file: far more than a file has, so only a runaway answer is refused. */
const MAX_NODES = 200_000;
const KINDS: readonly string[] = ['runStart', 'beforeRevert', 'beforeRestore', 'manual'];

type Rec = Record<string, unknown>;
const rec = (v: unknown): Rec | null => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Rec) : null);
/** An own property only: a value that sits on the prototype chain is not the sender's. */
const own = (o: Rec, k: string): unknown => (Object.prototype.hasOwnProperty.call(o, k) ? o[k] : undefined);
const int = (v: unknown, min: number, max: number): v is number => typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max;
const str = (v: unknown, max: number): v is string => typeof v === 'string' && v.length >= 1 && v.length <= max;

/** A row of the list, of a create or of a frame: null when anything is missing, mistyped or out of bound. */
function row(o: Rec): CheckpointRow | null {
  const n = own(o, 'n'), seq = own(o, 'seq'), kind = own(o, 'kind'), atMs = own(o, 'atMs');
  const label = own(o, 'label'), about = own(o, 'about'), by = own(o, 'by');
  if (!int(n, 1, U32) || !int(seq, 0, BIG) || !int(atMs, 0, BIG) || typeof kind !== 'string' || !KINDS.includes(kind)) return null;
  if (label !== undefined && (typeof label !== 'string' || label.length > MAX_LABEL)) return null;
  if (about !== undefined && !str(about, MAX_ACTOR)) return null;
  if (by !== undefined && !str(by, MAX_ACTOR)) return null;
  return {
    n, seq, kind: kind as CheckpointKind, atMs,
    ...(label !== undefined ? { label } : {}),
    ...(about !== undefined ? { about } : {}),
    ...(by !== undefined ? { by } : {}),
  };
}

/** The flat `checkpoint` frame of the socket as `Frame::Checkpoint`: the row nested, `type` and `room` dropped. */
export function toCheckpointFrame(m: unknown): Extract<Frame, { type: 'checkpoint' }> | null {
  const o = rec(m);
  const checkpoint = o && row(o);
  return checkpoint ? { type: 'checkpoint', checkpoint } : null;
}

const failed = (request: CheckpointRequest, code: string): Action => ({ type: 'checkpointRequestFailed', request, code });
const codeOf = (body: unknown): string => {
  const o = rec(body);
  const e = o ? rec(own(o, 'error')) : null;
  return safeCode(e ? own(e, 'code') : undefined);
};

/** The answer of `GET .../checkpoints` as `checkpointsLoaded`. A 404 with no error is a server that has no such route. */
export function answerOfRows(status: number, body: unknown): Action {
  const list: CheckpointRequest = { type: 'list' };
  if (status === 404 && !rec(body)?.error) return failed(list, 'not_found');
  if (status !== 200) return failed(list, codeOf(body));
  const o = rec(body);
  const rows = o ? own(o, 'checkpoints') : undefined;
  const canCreate = o ? own(o, 'canCreate') : undefined;
  if (!Array.isArray(rows) || rows.length > MAX_ROWS || typeof canCreate !== 'boolean') return failed(list, 'unavailable');
  const checkpoints: CheckpointRow[] = [];
  for (const r of rows) {
    const c = rec(r) && row(rec(r)!);
    if (!c) return failed(list, 'unavailable');
    checkpoints.push(c);
  }
  return { type: 'checkpointsLoaded', checkpoints, canCreate };
}

/** The answer of `POST .../checkpoints`, `{checkpoint, created}`, as `checkpointPosted` with the key the post carried. */
export const answerOfPost = (key: string) => (status: number, body: unknown): Action => {
  const request: CheckpointRequest = { type: 'post', key };
  if (status !== 200) return failed(request, codeOf(body));
  const o = rec(body);
  const r = o ? rec(own(o, 'checkpoint')) : null;
  const checkpoint = r && row(r);
  return checkpoint ? { type: 'checkpointPosted', key, checkpoint } : failed(request, 'unavailable');
};

/** The answer of `GET .../checkpoints/{n}/state`, `{seq, nodes}`, as `checkpointStateLoaded` (the app checks the nodes). */
export const answerOfState = (n: number) => (status: number, body: unknown): Action => {
  const request: CheckpointRequest = { type: 'state', n };
  if (status !== 200) return failed(request, codeOf(body));
  const o = rec(body);
  const seq = o ? own(o, 'seq') : undefined;
  const nodes = o ? own(o, 'nodes') : undefined;
  if (!int(seq, 0, BIG) || !Array.isArray(nodes) || nodes.length > MAX_NODES || !nodes.every((x) => rec(x))) return failed(request, 'unavailable');
  return { type: 'checkpointStateLoaded', n, seq, nodes: nodes as never };
};

type CheckpointEffect = Extract<Effect, { type: 'fetchCheckpoints' | 'postCheckpoint' | 'fetchCheckpointState' }>;

/** One request of the checkpoint routes: how to send it, how to read the answer, and what a network failure is. */
export interface CheckpointCall {
  method: 'GET' | 'POST';
  path: string;
  body?: unknown;
  answer: (status: number, body: unknown) => Action;
  failure: Action;
}

/** The request an effect stands for. A null label or `about` is left out of the body. */
export function checkpointCall(file: string, e: CheckpointEffect): CheckpointCall {
  const path = `/v1/files/${encodeURIComponent(file)}/checkpoints`;
  switch (e.type) {
    case 'fetchCheckpoints':
      return { method: 'GET', path, answer: answerOfRows, failure: failed({ type: 'list' }, 'unavailable') };
    case 'postCheckpoint': {
      const body = { key: e.key, kind: e.kind, seq: e.seq, ...(e.label !== null ? { label: e.label } : {}), ...(e.about !== null ? { about: e.about } : {}) };
      return { method: 'POST', path, body, answer: answerOfPost(e.key), failure: failed({ type: 'post', key: e.key }, 'unavailable') };
    }
    case 'fetchCheckpointState': {
      const n = Math.trunc(e.n);
      return { method: 'GET', path: `${path}/${n}/state`, answer: answerOfState(n), failure: failed({ type: 'state', n }, 'unavailable') };
    }
  }
}
