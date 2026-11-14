// A sample consumer that mirrors how the web client uses the protocol. If the Rust types change and this file does
// not follow, `tsc` fails here (mostly on the exhaustive switch below).
import type { ClientMsg, Command, Node, NodePatch, ServerMsg } from './index';

export const patch: NodePatch = { x: 10, w: 200, sw: 'fill' };
export const cmd: Command = { type: 'updateNode', id: 'n1', patch };
export const op: ClientMsg = { type: 'op', id: 'm1', idempotencyKey: 'k1', room: 'file:abc', command: cmd };

export const demoNode: Node = {
  id: 'n2', name: 'Box', type: 'rect', parentId: null,
  x: 0, y: 0, w: 120, h: 80, sw: 'fixed', sh: 'fixed',
  fill: '#8b5cf6', fill2: '#22d3ee', radius: 8,
  layout: 'none', gap: 8, padding: 16, align: 'start', justify: 'start',
  clip: true, text: '', fontSize: 16, shader: 'aurora',
  fontWeight: 400, textAlign: 'left', border: { width: 0, color: '#000000', position: 'inside' }, shadows: [],
};

/** Handles a server message. A new variant in Rust fails to compile here until it is handled. */
export function handle(m: ServerMsg): string {
  switch (m.type) {
    case 'connected': return m.connectionId;
    case 'subscribed': return m.room;
    case 'ack': return String(m.seq);
    case 'event': return `${m.actor}@${m.seq}`;
    case 'replayOverflow': return String(m.latestSeq);
    case 'error': return m.message;
    case 'pong': return 'pong';
    case 'captureRequest': return m.requestId;
    default: { const unreachable: never = m; return unreachable; }
  }
}
