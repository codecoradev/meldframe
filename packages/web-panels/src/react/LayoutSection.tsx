// React copy of LayoutSection.svelte. The Layout section of a frame as the Properties and Fill-and-layout boards draw it: free, row
// or column; for a row or a column the gap, where the children sit across and along it, and the padding (one value for all
// sides, and each side). It draws the document and reports input as actions (see layout.ts); it holds no state of its own.
import { Field } from '@meldframe/web-chrome/react/Field';
import { Section } from '@meldframe/web-chrome/react/Section';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { ALIGNS, JUSTIFIES, LAYOUT_MODES, SIDES, alignAction, justifyAction, layoutAction } from '@meldframe/web-document/layout';
import { textAction } from '@meldframe/web-document/properties';
import { type SectionProps, sender } from '../fields';

export function LayoutSection({ properties: p, dispatch }: SectionProps) {
  const send = sender(dispatch);
  // The app layer says the value the four sides share; when they differ the field is empty and says Mixed.
  const all = p.paddingUniform;
  return (
    <Section title="Layout">
      <Segmented label="Layout direction" options={LAYOUT_MODES} value={p.layout} onChange={(v) => send(layoutAction(p, v as typeof p.layout))} data-testid="prop-layout" />
      {p.layout !== 'none' ? (
        <>
          <div className="grid grid-cols-2 gap-2">
            <Field label="Gap" prefix="Gap" suffix="px" mono value={String(p.gap)} onCommit={(raw) => send(textAction(p, { type: 'gap' }, raw))} testId="prop-gap" />
            <Field label="Padding, all sides" prefix="Pad" suffix="px" mono value={all === undefined ? '' : String(all)} placeholder={all === undefined ? 'Mixed' : undefined} onCommit={(raw) => send(textAction(p, { type: 'paddingAll' }, raw))} testId="prop-padding" />
          </div>
          <div className="grid grid-cols-2 gap-2">
            {SIDES.map((s) => (
              <Field key={s.side} label={`Padding ${s.label.toLowerCase()}`} prefix={s.label[0]} mono value={String(p.padding[s.side])} onCommit={(raw) => send(textAction(p, { type: 'padding', side: s.side }, raw))} testId={`prop-padding-${s.side}`} />
            ))}
          </div>
          <span className="text-text-muted">Align</span>
          <Segmented label="Align children" options={ALIGNS} value={p.align} onChange={(v) => send(alignAction(p, v as typeof p.align))} data-testid="prop-align" />
          <span className="text-text-muted">Justify</span>
          <Segmented label="Justify children" options={JUSTIFIES} value={p.justify} onChange={(v) => send(justifyAction(p, v as typeof p.justify))} data-testid="prop-justify" />
        </>
      ) : null}
    </Section>
  );
}
