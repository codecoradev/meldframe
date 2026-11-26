// React copy of PropertiesPanel.svelte. The properties panel (the Design side of the right card): the values of the one selected
// node in the sections of the Properties and Inspector boards. A field reports its text when the person leaves it or presses Enter;
// what that becomes (an action, or nothing, and then the field shows the document's value again) is in properties.ts. It holds no
// state of its own. The Svelte switch file that hosts this panel (apps/web/src/react-pilot/PropertiesPanelSwitch.svelte) gives it
// the thumbnail route, which is the host's (see FillSection.tsx).
import type { Action, AssetsState, Picker, Properties } from '@meldframe/protocol';
import { Checkbox } from '@meldframe/web-chrome/react/Checkbox';
import { Field } from '@meldframe/web-chrome/react/Field';
import { Icon } from '@meldframe/web-chrome/react/Icon';
import { Section } from '@meldframe/web-chrome/react/Section';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { type PlainField, bindingOf, clipAction, percent, textAction, unbindAction } from '@meldframe/web-document/properties';
import { Fragment } from 'react';
import { round2, sender } from '../fields';
import { KIND_ICON } from '../layers';
import { BindPicker } from './BindPicker';
import { BorderSection } from './BorderSection';
import { BoundChip } from './BoundChip';
import { FillSection, type FillSectionProps } from './FillSection';
import { LayoutSection } from './LayoutSection';
import { PlacementSection } from './PlacementSection';
import { ShadowSection } from './ShadowSection';
import { SizeModes } from './SizeModes';
import { TextStyle } from './TextStyle';

export interface PropertiesPanelProps extends Pick<FillSectionProps, 'thumbnail'> {
  properties: Properties | null;
  /** The bind picker of the app state, open on at most one field. */
  picker?: Picker | null;
  /** The image picker of the app state (open for at most one node) and the images it has read. */
  assets?: AssetsState;
  /** How many nodes are selected: the empty state says which case it is. */
  selected: number;
  dispatch: (a: Action) => void;
}

export function PropertiesPanel({ properties: p, picker = null, assets, selected, dispatch, thumbnail }: PropertiesPanelProps) {
  const send = sender(dispatch);
  // What is typed goes to the app layer as typed: it parses, holds the value to the range of the field and sends nothing when nothing changes.
  const text = (q: Properties, field: PlainField['type']) => (raw: string) => send(textAction(q, { type: field }, raw));
  return (
    // An island is a root of its own: the provider the Svelte layout gives the buttons from above is here.
    <TooltipProvider>
      <div className="flex flex-col border-t border-hairline text-small text-text" role="group" aria-label="Properties" data-testid="properties">
        {p ? <Body p={p} picker={picker} assets={assets} dispatch={dispatch} text={text} thumbnail={thumbnail} /> : <Empty selected={selected} />}
      </div>
    </TooltipProvider>
  );
}

function Empty({ selected }: { selected: number }) {
  return (
    <div className="flex flex-col gap-1 px-4 py-4" data-testid="properties-empty">
      <h3 className="m-0 text-body font-semibold text-text-strong">{selected > 1 ? `${selected} nodes selected` : 'Nothing selected'}</h3>
      <p className="m-0 text-text-muted">{selected > 1 ? 'Select one node to edit its properties.' : 'Select a frame, a shape or text on the canvas to edit it.'}</p>
    </div>
  );
}

interface BodyProps extends Pick<FillSectionProps, 'thumbnail'> {
  p: Properties;
  picker: Picker | null;
  assets?: AssetsState;
  dispatch: (a: Action) => void;
  text: (q: Properties, field: PlainField['type']) => (raw: string) => void;
}

function Body({ p, picker, assets, dispatch, text, thumbnail }: BodyProps) {
  const send = sender(dispatch);
  const has = p.sections;
  const opacity = bindingOf(p, 'opacity');
  const radius = bindingOf(p, 'radius');
  return (
    // A new node is new fields: nothing typed for the last one carries over (the Svelte panel has a `{#key p.id}` here).
    <Fragment key={p.id}>
      <header className="flex items-center gap-2 px-4 pt-4 text-text-muted">
        <Icon name={KIND_ICON[p.kind]} />
        <div className="min-w-0 flex-1">
          <Field label="Name" value={p.name} onCommit={text(p, 'name')} testId="prop-name" />
        </div>
      </header>
      <Section title="Position">
        <div className="grid grid-cols-2 gap-2">
          <Field label="X" prefix="X" mono value={round2(p.x)} onCommit={text(p, 'x')} testId="prop-x" disabled={p.positionLocked} describedBy={p.positionLocked ? 'prop-position-note' : undefined} />
          <Field label="Y" prefix="Y" mono value={round2(p.y)} onCommit={text(p, 'y')} testId="prop-y" disabled={p.positionLocked} describedBy={p.positionLocked ? 'prop-position-note' : undefined} />
        </div>
        {p.positionLocked ? (
          // The position is the layout's: the fields show where the node is drawn and cannot move it.
          <p id="prop-position-note" className="m-0 text-caption text-text-muted">
            Set by the auto layout of the parent.
          </p>
        ) : null}
      </Section>
      <Section title="Size">
        <div className="grid grid-cols-2 gap-2">
          <Field label="Width" prefix="W" suffix="px" mono value={round2(p.w)} onCommit={text(p, 'w')} testId="prop-w" />
          <Field label="Height" prefix="H" suffix="px" mono value={round2(p.h)} onCommit={text(p, 'h')} testId="prop-h" />
        </div>
        <SizeModes properties={p} dispatch={dispatch} />
      </Section>
      {has.layout ? <LayoutSection properties={p} dispatch={dispatch} /> : null}
      {has.placement ? <PlacementSection properties={p} dispatch={dispatch} /> : null}
      <Section title="Appearance">
        <div className="grid grid-cols-2 items-end gap-2">
          {opacity ? (
            <div className="col-span-2">
              <BoundChip label="Opacity" field="opacity" binding={opacity} value={`${percent(p.opacity)} %`} onDetach={() => send(unbindAction(p, 'opacity'))} />
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              <span className="flex items-center justify-between text-text-muted">
                Opacity
                <BindPicker properties={p} field="opacity" picker={picker} dispatch={dispatch} />
              </span>
              <Field label="Opacity" suffix="%" value={percent(p.opacity)} onCommit={text(p, 'opacity')} testId="prop-opacity" />
            </div>
          )}
          {has.radius ? (
            radius ? (
              <div className="col-span-2">
                <BoundChip label="Corner radius" field="radius" binding={radius} value={round2(p.radius)} onDetach={() => send(unbindAction(p, 'radius'))} />
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                <span className="flex items-center justify-between text-text-muted">
                  Corner radius
                  <BindPicker properties={p} field="radius" picker={picker} dispatch={dispatch} />
                </span>
                <Field label="Corner radius" value={round2(p.radius)} onCommit={text(p, 'radius')} testId="prop-radius" />
              </div>
            )
          ) : null}
        </div>
        {has.clip ? <Checkbox label="Clip content" checked={p.clip} onChange={(c) => send(clipAction(p, c))} data-testid="prop-clip" /> : null}
      </Section>
      {has.fontSize ? (
        <Section title="Text">
          <Field label="Font size" suffix="px" value={round2(p.fontSize)} onCommit={text(p, 'fontSize')} testId="prop-fontSize" />
          <TextStyle properties={p} dispatch={dispatch} />
        </Section>
      ) : null}
      <FillSection properties={p} picker={picker} assets={assets} dispatch={dispatch} thumbnail={thumbnail} />
      <BorderSection properties={p} dispatch={dispatch} />
      <ShadowSection properties={p} dispatch={dispatch} />
    </Fragment>
  );
}
