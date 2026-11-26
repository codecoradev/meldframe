// React copy of FillSection.svelte. The Fill section as the Properties, Inspector and Fill-and-layout boards draw it: the type switch
// (solid, linear, radial, image, none), then the fields of the type: a colour with the bind button (or the chip of the variable it
// is bound to), or a gradient with its angle, a bar, its stops and a button to add one. The Image type opens the image picker (the
// workspace's images, ImageFillPicker; open while the app state says so); an image fill shows its thumbnail and short id, a button to
// change it and its fit. It draws the document and reports input as actions (see fills.ts and images.ts).
import type { AssetsState, Picker } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Field } from '@meldframe/web-chrome/react/Field';
import { IconButton } from '@meldframe/web-chrome/react/IconButton';
import { Section } from '@meldframe/web-chrome/react/Section';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import { addStopAction, fillTypeAction, gradientBar, removeStopAction, stopPercent } from '@meldframe/web-document/fills';
import { bindingOf, textAction, unbindAction } from '@meldframe/web-document/properties';
import type { ImageFit } from '@meldframe/protocol';
import { useEffect, useRef } from 'react';
import { type SectionProps, pickerClosed, round2, sender } from '../fields';
import { IMAGE_FITS, fillTypes, fitAction, openAssetsAction, shortId } from '../images';
import { BindPicker } from './BindPicker';
import { BoundChip } from './BoundChip';
import { ImageFillPicker } from './ImageFillPicker';

const CLOSED: AssetsState = { items: [], loading: false, loaded: false, capped: false, request: 0 };
/** The page gave no route for thumbnails: no image is drawn, and a tile says so. */
const NO_ROUTE = () => null;

export interface FillSectionProps extends SectionProps {
  picker: Picker | null;
  /** The image picker of the app state: open for the node it names, with the images read so far. */
  assets?: AssetsState;
  /**
   * Where the thumbnail of an image fill, and of each tile of the image picker, is fetched from, or null for an id that is not an
   * asset id (the file is the sender's, not trusted); the page gives it, because it is the host package that knows the route
   * (web-assets), which a view may not import.
   */
  thumbnail?: (assetId: string) => string | null;
}

export function FillSection({ properties: p, picker, assets = CLOSED, dispatch, thumbnail }: FillSectionProps) {
  const send = sender(dispatch);
  const fill = bindingOf(p, 'fill');
  // What the node can take is the app layer's to say (gradientAllowed, imageAllowed).
  const types = fillTypes(p);
  const open = assets.target === p.id;
  const choose = (k: string) => send(k === 'image' ? openAssetsAction(p) : fillTypeAction(p, k as typeof p.fillKind));
  // The picker closed for this node (a choice, Escape or its button) and focus fell to the page: put it back on the switch.
  // A picker that closed because another node was selected, or because the person clicked somewhere else, leaves focus alone.
  const host = useRef<HTMLDivElement>(null);
  const was = useRef<string | null>(null);
  useEffect(() => {
    const closed = pickerClosed(was.current, p.id, open);
    was.current = open ? p.id : null;
    if (!closed) return;
    queueMicrotask(() => {
      if (document.activeElement && document.activeElement !== document.body) return;
      host.current?.querySelector<HTMLElement>('[role=radio][aria-checked=true]')?.focus();
    });
  }, [open, p.id]);
  const swatch = p.fill === 'none' ? 'transparent' : p.fill;
  const g = p.gradient;
  const thumb = p.image ? thumbnail?.(p.image.assetId) : null;
  return (
    <Section title="Fill">
      <div ref={host} className="contents">
        <Segmented label="Fill type" options={types} value={p.fillKind} onChange={choose} data-testid="prop-fill-type" />
      </div>
      {open ? <ImageFillPicker properties={p} assets={assets} dispatch={dispatch} thumbnail={thumbnail ?? NO_ROUTE} /> : null}
      {p.fillKind === 'solid' || p.fillKind === 'none' || p.fillKind === 'image' ? (
        <>
          {p.fillKind === 'image' && p.image ? (
            <>
              <div className="flex items-center gap-2" data-testid="prop-fill-image">
                <span className="block size-8 shrink-0 overflow-hidden rounded-sm border border-border bg-field">{thumb ? <img src={thumb} alt="" decoding="async" className="size-full object-contain" /> : null}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-caption" title={p.image.assetId}>
                  {`Image ${shortId(p.image.assetId)}`}
                </span>
                <Button size="sm" onClick={() => send(openAssetsAction(p))} data-testid="prop-fill-image-change">
                  Change image
                </Button>
              </div>
              <Segmented label="Image fit" options={IMAGE_FITS} value={p.image.fit} onChange={(f) => send(fitAction(p, f as ImageFit))} data-testid="prop-fill-image-fit" />
            </>
          ) : null}
          {p.fillKind === 'solid' ? (
            fill ? (
              <BoundChip label="" field="fill" binding={fill} value={p.fill} onDetach={() => send(unbindAction(p, 'fill'))} />
            ) : (
              <div className="flex items-center gap-2">
                <div className="min-w-0 flex-1">
                  <Field
                    label="Fill"
                    value={p.fill}
                    onCommit={(raw) => send(textAction(p, { type: 'fill' }, raw))}
                    testId="prop-fill"
                    lead={<i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: swatch }}></i>}
                  />
                </div>
                <BindPicker properties={p} field="fill" picker={picker} dispatch={dispatch} />
              </div>
            )
          ) : null}
        </>
      ) : g ? (
        <>
          {p.fillKind === 'linear' ? (
            <div className="flex items-center gap-2">
              <span className="w-10 text-text-muted">Angle</span>
              <div className="w-24">
                <Field label="Angle" suffix="°" mono value={round2(g.angle)} onCommit={(raw) => send(textAction(p, { type: 'gradientAngle' }, raw))} testId="prop-gradient-angle" />
              </div>
            </div>
          ) : null}
          <div className="h-4 rounded-sm border border-border" style={{ background: gradientBar(g) }} data-testid="prop-gradient-bar" role="img" aria-label={`Gradient, ${g.stops.length} stops`}></div>
          <div className="flex flex-col gap-1">
            {g.stops.map((s, i) => (
              <div key={i} className="grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-center gap-2" data-testid="prop-gradient-stop">
                <i className="block size-4 shrink-0 rounded-sm border border-border" style={{ background: s.color }}></i>
                <Field label={`Stop ${i + 1} colour`} size="dense" mono value={s.color} onCommit={(raw) => send(textAction(p, { type: 'stopColor', index: i }, raw))} testId="prop-gradient-color" />
                <div className="w-24">
                  <Field label={`Stop ${i + 1} position`} size="dense" mono suffix="%" value={stopPercent(s.pos)} onCommit={(raw) => send(textAction(p, { type: 'stopPos', index: i }, raw))} testId="prop-gradient-pos" />
                </div>
                <IconButton label={`Remove stop ${i + 1}`} icon="minus" size={24} disabled={g.stops.length <= p.minStops} onClick={() => send(removeStopAction(p, i))} data-testid="prop-gradient-remove" />
              </div>
            ))}
          </div>
          <div className="flex">
            <Button size="sm" disabled={g.stops.length >= p.maxStops} onClick={() => send(addStopAction(p))} data-testid="prop-gradient-add">
              Add stop
            </Button>
          </div>
        </>
      ) : null}
    </Section>
  );
}
