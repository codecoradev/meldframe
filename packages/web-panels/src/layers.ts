// What the Layers panel draws from a row, apart from the markup: the icon of a kind and who changed the node last (the name and
// the class of the dot). Both copies of the panel (Svelte and React) read these, so the maps and the rule are not written twice.
// Presentation only: which actor changed a node is the file's, `actorLabel` and `actorTone` are the helpers of `web-document`.
import type { IconName } from '@meldframe/web-chrome/icons';
import { actorLabel, actorTone, type LayerRow } from '@meldframe/web-document/layers';

export const KIND_ICON: Record<LayerRow['kind'], IconName> = { frame: 'frame', rect: 'rectangle', ellipse: 'ellipse', text: 'text', shader: 'layers' };

const AGENT_DOT = { 1: 'bg-agent-1', 2: 'bg-agent-2', 3: 'bg-agent-3' } as const;

/** The actor to name on a row: none for the page's own changes (`self`) and for a node nobody changed; `dot` is the class that draws its colour. */
export function changedBy(r: LayerRow, self: string): { name: string; dot: string } | null {
  if (!r.actor || r.actor === self) return null;
  const who = actorLabel(r.actor);
  return { name: who.name, dot: who.kind === 'agent' ? AGENT_DOT[actorTone(r.actor)] : 'bg-person' };
}
