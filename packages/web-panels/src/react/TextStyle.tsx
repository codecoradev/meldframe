// React copy of TextStyle.svelte. The weight and the alignment of a text, in the Text section under the font size: a choice of the
// nine weights the model takes and of left, center or right, each under a caption as the fields above are. It draws the document
// and reports input as actions (see properties.ts); it holds no state of its own, and no default of its own. The weight is a native select, so the keyboard
// and the screen reader get it as they know it. It is a controlled select: a weight the document did not take is put back by
// React after the handler, so the select shows the document's value again.
import { ring } from '@meldframe/web-chrome/focus';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { FONT_WEIGHTS, TEXT_ALIGNS, fontWeightAction, textAlignAction } from '@meldframe/web-document/properties';
import type { TextAlign } from '@meldframe/protocol';
import { type SectionProps, sender } from '../fields';

const select = `${ring} h-(--mf-size-control-lg) w-full rounded-md border border-divider bg-field px-2 text-small text-text hov:border-border-strong`;

export function TextStyle({ properties: p, dispatch }: SectionProps) {
  const send = sender(dispatch);
  // The app layer sends both for a text and neither for any other kind: no default is made here, and a node without them has no text style.
  if (p.fontWeight === undefined || p.textAlign === undefined) return null;
  return (
    <>
      <div className="flex flex-col gap-2">
        <label htmlFor="prop-fontWeight" className="text-text-muted">
          Font weight
        </label>
        <select id="prop-fontWeight" className={select} value={String(p.fontWeight)} onChange={(e) => send(fontWeightAction(p, e.currentTarget.value))} data-testid="prop-fontWeight">
          {FONT_WEIGHTS.map((w) => (
            <option key={w.value} value={w.value}>
              {w.label}
            </option>
          ))}
        </select>
      </div>
      <div className="flex flex-col gap-2">
        <span className="text-text-muted">Text alignment</span>
        <Segmented label="Text alignment" iconOnly options={TEXT_ALIGNS} value={p.textAlign} onChange={(v) => send(textAlignAction(p, v as TextAlign))} data-testid="prop-textAlign" />
      </div>
    </>
  );
}
