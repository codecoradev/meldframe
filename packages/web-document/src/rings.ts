// The touch rings on the canvas: the app layer keeps the rings (`state.activity.touches`: which nodes an actor is
// changing, for how long, and none for this page's own actor); this turns them into what the canvas draws. Pure.
import type { TouchRing } from '@meldframe/protocol';
import { actorLabel } from './layers';

export interface Touched {
  id: string;
  /** `0xRRGGBB` of the actor. */
  color: number;
  label: string;
  nodeIds: string[];
  /** When it came, on the clock of the canvas (`performance.now()`); `Infinity` for a ring that stays while its lane works. */
  at: number;
}

export interface Palette {
  person: number;
  agents: readonly number[];
}

/** The two clocks at one instant: the shell's (epoch milliseconds, which stamps the state) and the canvas's. */
export interface Clock {
  epochMs: number;
  perfMs: number;
}

/** A person has one colour; an agent keeps the same one of the agent colours, picked from its id. */
export function actorColor(actor: string, palette: Palette): number {
  if (actorLabel(actor).kind === 'person') return palette.person;
  let h = 0;
  for (const c of actor) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return palette.agents[h % palette.agents.length]!;
}

/** The rings to draw: each on the canvas clock by its age, so a ring fades from when it came and not from when it is drawn. */
export function ringsOnCanvas(rings: readonly TouchRing[], clock: Clock, palette: Palette): Touched[] {
  return rings.flatMap((r, i) =>
    r.ids.length === 0
      ? []
      : [{
          id: `${r.actor}#${r.atMs}#${i}`,
          color: actorColor(r.actor, palette),
          label: actorLabel(r.actor).name,
          nodeIds: [...r.ids],
          // An age is never negative: a clock that disagrees must not keep a ring at full strength for longer.
          at: r.persistent ? Number.POSITIVE_INFINITY : Math.min(clock.perfMs, clock.perfMs - (clock.epochMs - r.atMs)),
        }],
  );
}
