// The touch rings: a ring flashes on the nodes another actor just changed. The server sends an ephemeral `touch`
// frame; this keeps the few rings that are still visible. Pure.
import { actorLabel } from './layers';

export interface Touched {
  id: string;
  /** `0xRRGGBB` of the actor. */
  color: number;
  label: string;
  nodeIds: string[];
  /** When it happened, in the milliseconds of the clock the caller uses. */
  at: number;
}

export interface Palette {
  person: number;
  agents: readonly number[];
}

export interface TouchOptions {
  /** The actor of this page: its own changes get no ring. */
  self: string;
  palette: Palette;
  /** A ring older than this is gone (the canvas has faded it by then). */
  maxAgeMs: number;
}

const MAX_RINGS = 50;

/** A person has one colour; an agent keeps the same one of the agent colours, picked from its id. */
export function actorColor(actor: string, palette: Palette): number {
  if (actorLabel(actor).kind === 'person') return palette.person;
  let h = 0;
  for (const c of actor) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return palette.agents[h % palette.agents.length]!;
}

let serial = 0;

/** The rings after a touch: the new one added, the old ones dropped. */
export function addTouch(list: readonly Touched[], t: { actor: string; nodeIds: readonly string[] }, now: number, o: TouchOptions): Touched[] {
  const alive = list.filter((x) => now - x.at < o.maxAgeMs);
  if (t.actor === o.self || t.nodeIds.length === 0) return alive;
  const ring: Touched = {
    id: `${t.actor}#${++serial}`,
    color: actorColor(t.actor, o.palette),
    label: actorLabel(t.actor).name,
    nodeIds: [...t.nodeIds],
    at: now,
  };
  return [...alive, ring].slice(-MAX_RINGS);
}
