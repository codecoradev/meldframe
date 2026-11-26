// React copy of SizeModes.svelte. How the width and the height of the node are sized (fixed, hug or fill), under the Size
// fields, and the notes that say a choice has no effect where the node is. The notes are the app layer's; this words them. It
// draws the document and reports input as actions (see layout.ts); it holds no state of its own.
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { SIZE_MODES, noteText, sizingAction } from '@meldframe/web-document/layout';
import { type SectionProps, sender } from '../fields';

export function SizeModes({ properties: p, dispatch }: SectionProps) {
  const send = sender(dispatch);
  return (
    <div className="flex flex-col gap-2">
      <span className="text-text-muted">Width sizing</span>
      <Segmented label="Width sizing" options={SIZE_MODES} value={p.sw} onChange={(v) => send(sizingAction(p, 'width', v as typeof p.sw))} data-testid="prop-sw" />
      <span className="text-text-muted">Height sizing</span>
      <Segmented label="Height sizing" options={SIZE_MODES} value={p.sh} onChange={(v) => send(sizingAction(p, 'height', v as typeof p.sh))} data-testid="prop-sh" />
      {p.notes.map((note, i) => (
        <p key={i} className="m-0 text-caption text-text-muted" role="note" data-testid="prop-note">
          {noteText(note)}
        </p>
      ))}
    </div>
  );
}
