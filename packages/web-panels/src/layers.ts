// What the Layers panel draws from a row, apart from the markup: the icon of a kind and who changed the node
// last (the name and the class of the dot). Both copies of the panel (Svelte and React) read these, so the maps and the rule are
// not written twice. Presentation only: a row is `AppState.layers`, whose name and actor the app layer made safe to show once
// (rule U9), and the tone of an actor is the app layer's (`ActorView.tone`); this maps it to a class and works nothing out.
import type { ActorTone, LayerRow } from '@meldframe/protocol';
import type { IconName } from '@meldframe/web-chrome/icons';

export const KIND_ICON: Record<LayerRow['kind'], IconName> = { frame: 'frame', rect: 'rectangle', ellipse: 'ellipse', text: 'text', shader: 'layers' };

const DOT: Record<ActorTone, string> = { person: 'bg-person', agent1: 'bg-agent-1', agent2: 'bg-agent-2', agent3: 'bg-agent-3' };

/** The actor to name on a row: none for the page's own changes (`self`) and for a node nobody changed; `dot` is the class that draws its colour. */
export function changedBy(r: LayerRow, self: string): { name: string; tone: ActorTone; dot: string } | null {
  if (!r.actor || r.actor.actor === self) return null;
  return { name: r.actor.name, tone: r.actor.tone, dot: DOT[r.actor.tone] };
}
