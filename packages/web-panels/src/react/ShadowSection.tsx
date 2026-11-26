// React copy of ShadowSection.svelte. The Shadow section as the Properties board draws it: a row for each drop shadow (offset,
// blur, spread and colour, in the order they are drawn) with a button to remove it, and a button to add one, up to the limit the
// app layer gives. It draws the document and reports input as actions (see decoration.ts); it holds no state of its own.
import { Button } from '@meldframe/web-chrome/react/Button';
import { Field } from '@meldframe/web-chrome/react/Field';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { Section } from '@meldframe/web-chrome/react/Section';
import { SHADOW_NUMBERS, addShadowAction, removeShadowAction, shadowAction } from '@meldframe/web-document/decoration';
import { type SectionProps, round2, sender } from '../fields';

export function ShadowSection({ properties: p, dispatch }: SectionProps) {
  const send = sender(dispatch);
  return (
    <Section title="Shadow">
      {p.shadows.map((s, i) => (
        <div key={i} className="flex flex-col gap-2" data-testid="prop-shadow">
          <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
            <Field
              label={`Shadow ${i + 1} colour`}
              mono
              value={s.color}
              onCommit={(raw) => send(shadowAction(p, i, 'color', raw))}
              testId="prop-shadow-color"
              lead={<i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: s.color }}></i>}
            />
            <IconButton label={`Remove shadow ${i + 1}`} icon="minus" size={24} onClick={() => send(removeShadowAction(p, i))} data-testid="prop-shadow-remove" />
          </div>
          <div className="grid grid-cols-2 gap-2">
            {SHADOW_NUMBERS.map((n) => (
              <Field key={n.field} label={`Shadow ${i + 1} ${n.label.toLowerCase()}`} prefix={n.label} mono value={round2(s[n.field])} onCommit={(raw) => send(shadowAction(p, i, n.field, raw))} testId={`prop-shadow-${n.field}`} />
            ))}
          </div>
        </div>
      ))}
      <div className="flex">
        <Button size="sm" disabled={p.shadows.length >= p.maxShadows} onClick={() => send(addShadowAction(p))} data-testid="prop-shadow-add">
          Add shadow
        </Button>
      </div>
    </Section>
  );
}
