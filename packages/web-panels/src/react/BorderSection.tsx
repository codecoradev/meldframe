// React copy of BorderSection.svelte. The Border section as the Properties and Fill-and-layout boards draw it: the width, the
// colour with its swatch, and where the border sits against the edge, and while there is a border a Remove button (the width 0
// the field sends). A width of 0 is no border. It draws the document and reports input as actions (see decoration.ts); it
// holds no state of its own.
import { Field } from '@meldframe/web-chrome/react/Field';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { Section } from '@meldframe/web-chrome/react/Section';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { BORDER_POSITIONS, borderPositionAction, removeBorderAction } from '@meldframe/web-document/decoration';
import { textAction } from '@meldframe/web-document/properties';
import { useRef } from 'react';
import { type SectionProps, round2, sender } from '../fields';

export function BorderSection({ properties: p, dispatch }: SectionProps) {
  const send = sender(dispatch);
  const on = p.border.width > 0;
  // The button goes away with the border, so focus moves first, to the width field (which stays and is what the button
  // changes), and the action is sent after: focus never falls to the page, whenever the document answers.
  const widths = useRef<HTMLDivElement>(null);
  const remove = () => {
    widths.current?.querySelector<HTMLElement>('[data-testid=prop-border-width]')?.focus();
    send(removeBorderAction(p));
  };
  return (
    <Section title="Border">
      <div ref={widths} className="grid grid-cols-2 gap-2">
        <Field label="Border width" suffix="px" mono value={round2(p.border.width)} onCommit={(raw) => send(textAction(p, { type: 'borderWidth' }, raw))} testId="prop-border-width" />
        <Field
          label="Border colour"
          mono
          value={p.border.color}
          onCommit={(raw) => send(textAction(p, { type: 'borderColor' }, raw))}
          testId="prop-border-color"
          lead={<i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: p.border.color }}></i>}
        />
      </div>
      {on ? (
        <div className="flex items-center gap-2">
          <div className="min-w-0 flex-1">
            <Segmented label="Border position" options={BORDER_POSITIONS} value={p.border.position} onChange={(v) => send(borderPositionAction(p, v as typeof p.border.position))} data-testid="prop-border-position" />
          </div>
          <IconButton label="Remove border" icon="minus" size={24} onClick={remove} data-testid="prop-border-remove" />
        </div>
      ) : null}
    </Section>
  );
}
