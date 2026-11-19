// Getting the file the editor opens, and what the page does when it cannot start.
import type { Node, Variables } from '@meldframe/protocol';

export const RELOAD_LIMIT = 3;
export const RELOAD_WINDOW_MS = 60_000;

interface Store {
  getItem(k: string): string | null;
  setItem(k: string, v: string): void;
}

/** True for at most RELOAD_LIMIT automatic reloads within a minute: a reload that diverges again must not loop. */
export function shouldReload(store: Store, now: number): boolean {
  try {
    const was = JSON.parse(store.getItem('meldframe-reloads') ?? 'null') as { count: number; at: number } | null;
    const count = was && now - was.at <= RELOAD_WINDOW_MS ? was.count : 0;
    if (count >= RELOAD_LIMIT) return false;
    store.setItem('meldframe-reloads', JSON.stringify({ count: count + 1, at: now }));
    return true;
  } catch {
    return false; // no storage, no way to bound the loop: do not reload by itself
  }
}

/** True when the app says that the file and the events no longer fit (and the limit of reloads is not used up). */
export function reloadOnDivergence(events: readonly { type: string }[], getStore: () => Store, now: number): boolean {
  if (!events.some((e) => e.type === 'diverged')) return false;
  try {
    return shouldReload(getStore(), now); // reaching the storage can throw (site data blocked)
  } catch {
    return false;
  }
}

export function describeSnapshotFailure(status: number, text: string): string {
  const hint =
    status === 403
      ? " The server's read policy forbids the snapshot."
      : status === 401
        ? ' The dev token is missing or wrong: run `just dev` first, then `just web`.'
        : '';
  return `Could not load the file: the server answered ${status}${text ? ` (${text.slice(0, 200)})` : ''}.${hint}`;
}

const NO_VARIABLES: Variables = { collections: [], variables: [] };

/** Who last changed a node, and at which sequence number: what the server adds to each node of a snapshot. */
export type Attribution = Record<string, { actor: string; seq: number }>;

export interface LoadedFile {
  /** The last server sequence number in `nodes`: the live stream continues after it. */
  seq: number;
  /** The nodes as the app layer takes them (it refuses fields it does not know). */
  nodes: Node[];
  /** The collections and variables of the file (empty when it has none): the canvas paints their resolved values. */
  variables: Variables;
  attribution: Attribution;
}

/**
 * The file as it is now. A file comes to exist with its first accepted op, so one the server does not know yet is an
 * empty file at sequence 0.
 */
export async function loadFile(get: typeof fetch, file: string): Promise<LoadedFile> {
  const res = await get(`/v1/files/${encodeURIComponent(file)}`);
  if (res.status === 404) return { seq: 0, nodes: [], variables: NO_VARIABLES, attribution: {} };
  if (!res.ok) throw new Error(describeSnapshotFailure(res.status, await res.text().catch(() => '')));
  const body: unknown = await res.json().catch(() => null);
  const b = body as { seq?: unknown; nodes?: unknown; variables?: Variables } | null;
  if (!b || typeof b.seq !== 'number' || !Number.isInteger(b.seq) || b.seq < 0 || !Array.isArray(b.nodes)) {
    throw new Error('Could not load the file: the snapshot is not a sequence number and a list of nodes.');
  }
  const attribution: Attribution = {};
  const nodes = (b.nodes as (Node & { lastActor?: unknown; lastSeq?: unknown })[]).map((raw) => {
    const { lastActor, lastSeq, ...node } = raw;
    if (typeof lastActor === 'string' && typeof lastSeq === 'number') attribution[node.id] = { actor: lastActor, seq: lastSeq };
    return node as Node;
  });
  return { seq: b.seq, nodes, variables: b.variables ?? NO_VARIABLES, attribution };
}
