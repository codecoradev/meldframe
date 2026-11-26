// The touch rings on the canvas: the app layer keeps the rings (`state.activity.touches`: which nodes an actor is
// changing, for how long, and none for this page's own actor) and the view of each actor (its name and its tone, the
// same value the layer rows and the lanes carry); this turns them into what the canvas draws. Pure. Nothing here derives
// a kind, a name or a tone from an actor string: a tone is only mapped to a colour.
import type { ActorTone, TouchRing } from '@meldframe/protocol';

export interface Touched {
  id: string;
  /** `0xRRGGBB` of the actor. */
  color: number;
  label: string;
  nodeIds: string[];
  /** When it came, on the clock of the canvas (`performance.now()`); `Infinity` for a ring that stays while its lane works. */
  at: number;
}

/** The colour of each tone of an actor (presentation: the tokens module gives the values). */
export type Palette = Readonly<Record<ActorTone, number>>;

/** The two clocks at one instant: the shell's (epoch milliseconds, which stamps the state) and the canvas's. */
export interface Clock {
  epochMs: number;
  perfMs: number;
}

/** The rings to draw: each on the canvas clock by its age, so a ring fades from when it came and not from when it is drawn. */
export function ringsOnCanvas(rings: readonly TouchRing[], clock: Clock, palette: Palette): Touched[] {
  return rings.flatMap((r, i) =>
    r.ids.length === 0
      ? []
      : [{
          id: `${r.actor}#${r.atMs}#${i}`,
          color: palette[r.view.tone],
          label: r.view.name,
          nodeIds: [...r.ids],
          // An age is never negative: a clock that disagrees must not keep a ring at full strength for longer.
          at: r.persistent ? Number.POSITIVE_INFINITY : Math.min(clock.perfMs, clock.perfMs - (clock.epochMs - r.atMs)),
        }],
  );
}
