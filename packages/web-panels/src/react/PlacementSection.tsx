// React copy of PlacementSection.svelte. The placement of a node inside an auto layout parent as the Fill-and-layout board draws
// it: its margin on each side, and whether it leaves the flow (absolute). Shown in an auto layout parent, and elsewhere only
// while the node has a margin or is absolute (the notes say they do nothing there, and these fields clear them). It draws the
// document and reports input as actions (see layout.ts); it holds no state of its own.
import { Checkbox } from '@meldframe/web-chrome/react/Checkbox';
import { Field } from '@meldframe/web-chrome/react/Field';
import { Section } from '@meldframe/web-chrome/react/Section';
import { SIDES, absoluteAction } from '@meldframe/web-document/layout';
import { textAction } from '@meldframe/web-document/properties';
import { type SectionProps, sender } from '../fields';

export function PlacementSection({ properties: p, dispatch }: SectionProps) {
  const send = sender(dispatch);
  return (
    <Section title="Margin">
      <div className="grid grid-cols-2 gap-2">
        {SIDES.map((s) => (
          <Field key={s.side} label={`Margin ${s.label.toLowerCase()}`} prefix={s.label[0]} mono value={String(p.margin[s.side])} onCommit={(raw) => send(textAction(p, { type: 'margin', side: s.side }, raw))} testId={`prop-margin-${s.side}`} />
        ))}
      </div>
      <Checkbox label="Absolute position" checked={p.absolute} onChange={(c) => send(absoluteAction(p, c))} data-testid="prop-absolute" />
    </Section>
  );
}
