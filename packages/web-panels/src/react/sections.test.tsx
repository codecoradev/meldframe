// The React copies of the Properties sections (Layout, Placement, Size modes, Text style, Border, Shadow), checked as layout.test.ts,
// decoration.test.ts and properties.test.ts check the Svelte ones: the markup of a server render, in Node. Typing, clicks and the
// focus are the browser scripts' (text-style-e2e.mjs, border-remove-e2e.mjs and editor-e2e.mjs with MELDFRAME_UI=react, which need the panel).
import type { Properties } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import BorderSectionSvelte from '../BorderSection.svelte';
import LayoutSectionSvelte from '../LayoutSection.svelte';
import PlacementSectionSvelte from '../PlacementSection.svelte';
import ShadowSectionSvelte from '../ShadowSection.svelte';
import SizeModesSvelte from '../SizeModes.svelte';
import TextStyleSvelte from '../TextStyle.svelte';
import { BorderSection } from './BorderSection';
import { LayoutSection } from './LayoutSection';
import { PlacementSection } from './PlacementSection';
import { ShadowSection } from './ShadowSection';
import { SizeModes } from './SizeModes';
import { TextStyle } from './TextStyle';
import { viewOf } from './compare';
import { FRAME, LINE_B, RECT, TEXT, properties, shadow } from './fixture';

const html = (el: ReactElement) => renderToStaticMarkup(<TooltipProvider>{el}</TooltipProvider>);
const dispatch = () => {};
const layout = (p: Properties) => html(<LayoutSection properties={p} dispatch={dispatch} />);
const placement = (p: Properties) => html(<PlacementSection properties={p} dispatch={dispatch} />);
const sizes = (p: Properties) => html(<SizeModes properties={p} dispatch={dispatch} />);
const textStyle = (p: Properties) => html(<TextStyle properties={p} dispatch={dispatch} />);
const border = (p: Properties) => html(<BorderSection properties={p} dispatch={dispatch} />);
const shadows = (p: Properties) => html(<ShadowSection properties={p} dispatch={dispatch} />);

// The labels of the options of a segmented control that are checked.
const checked = (out: string, group: string): string[] => {
  const from = out.slice(out.indexOf(`aria-label="${group}"`));
  return [...from.slice(0, from.indexOf('</div>')).matchAll(/aria-checked="true"[^>]*title="([^"]*)"/g)].map((m) => m[1]!);
};
const tag = (out: string, name: string, id: string) => out.match(new RegExp(`<${name}[^>]*data-testid="${id}"[^>]*>`))?.[0];
// The attribute, not the `disabled:` variants of the classes.
const isDisabled = (button: string) => /\sdisabled(?=[\s=>])/.test(button);

describe('the React Layout section', () => {
  it('shows the direction of a frame and, for a free frame, nothing more of the layout', () => {
    const out = layout(properties());
    expect(out).toContain('aria-label="Layout direction"');
    for (const w of ['Free', 'Row', 'Column']) expect(out).toContain(w);
    for (const id of ['prop-gap', 'prop-padding', 'prop-align', 'prop-justify']) expect(out).not.toContain(`data-testid="${id}"`);
  });
  it('shows the gap, the padding and the alignment of a row or a column', () => {
    const out = layout(properties({ layout: 'row' }));
    for (const id of ['prop-gap', 'prop-padding', 'prop-padding-top', 'prop-padding-right', 'prop-padding-bottom', 'prop-padding-left', 'prop-align', 'prop-justify']) expect(out).toContain(`data-testid="${id}"`);
    expect(out).toContain('Between');
  });
  it('shows the padding the sides share, and a Mixed placeholder in an empty field when they differ', () => {
    const same = tag(layout(properties({ layout: 'row', padding: { top: 4, right: 4, bottom: 4, left: 4 }, paddingUniform: 4 })), 'input', 'prop-padding')!;
    const mixed = tag(layout(properties({ layout: 'row', padding: { top: 4, right: 4, bottom: 4, left: 8 } })), 'input', 'prop-padding')!;
    expect(same).toContain('value="4"');
    expect(same).not.toContain('placeholder');
    expect(mixed).toContain('value=""');
    expect(mixed).toContain('placeholder="Mixed"');
  });
  it('marks the chosen mode, and the alignment of the children', () => {
    const out = layout(properties({ layout: 'column', align: 'center', justify: 'between' }));
    expect(checked(out, 'Layout direction')).toEqual(['Column']);
    expect(checked(out, 'Align children')).toHaveLength(1);
    expect(checked(out, 'Justify children')).toEqual(['Between']);
  });
});

describe('the React Size modes and Placement', () => {
  it('sizes the width and the height, the chosen mode checked', () => {
    const out = sizes(properties({ sw: 'hug' }));
    expect(out).toContain('aria-label="Width sizing"');
    expect(out).toContain('aria-label="Height sizing"');
    for (const w of ['Fixed', 'Hug', 'Fill']) expect(out).toContain(w);
    expect(checked(out, 'Width sizing')).toEqual(['Hug']);
    expect(checked(out, 'Height sizing')).toEqual(['Fixed']);
  });
  it('words each note the app layer sends, as a note, and has none otherwise', () => {
    const out = sizes(properties({ notes: [{ note: 'fillUnderHug', axis: 'width' }, { note: 'marginIgnored' }] }));
    expect(out.match(/role="note"/g)).toHaveLength(2);
    expect(out).toContain('Fill width needs a parent with auto layout');
    expect(out).toContain('The margin takes no room here');
    expect(sizes(properties())).not.toContain('role="note"');
  });
  it('has the margin of each side and Absolute position, with the value of each side', () => {
    const out = placement(properties({ kind: 'rect', sections: { ...RECT, placement: true }, margin: { top: 8, right: 0, bottom: 0, left: 0 }, absolute: true, notes: [{ note: 'marginIgnored' }] }));
    for (const s of ['top', 'right', 'bottom', 'left']) expect(out).toContain(`data-testid="prop-margin-${s}"`);
    expect(tag(out, 'input', 'prop-margin-top')).toContain('value="8"');
    expect(out).toContain('aria-label="Margin"');
    expect(out).toMatch(/<input[^>]*data-testid="prop-absolute"[^>]*checked/);
    expect(placement(properties({ sections: { ...FRAME, placement: true } }))).not.toMatch(/<input[^>]*data-testid="prop-absolute"[^>]*checked/);
  });
});

describe('the React text style', () => {
  const text = properties({ kind: 'text', sections: TEXT, fontWeight: 700, textAlign: 'center' });
  it('offers a weight, labelled, with the nine weights the model takes and the node\'s own chosen', () => {
    const out = textStyle(text);
    expect(out).toContain('data-testid="prop-fontWeight"');
    // The caption is the label of the select (one name, no aria-label over it).
    expect(out).toMatch(/<label for="prop-fontWeight"[^>]*>Font weight<\/label>/);
    expect(out).toMatch(/<select[^>]*id="prop-fontWeight"[^>]*>/);
    expect(out).not.toContain('aria-label="Font weight"');
    expect(out.match(/<option/g)).toHaveLength(9);
    expect(out).toMatch(/<option value="700" selected(="")?[^>]*>Bold<\/option>|<option[^>]*value="700"[^>]*selected[^>]*>Bold<\/option>/);
  });
  it('offers the alignment under a caption of its own, icon only, the node\'s own checked', () => {
    const out = textStyle(text);
    expect(out).toMatch(/<span[^>]*>Text alignment<\/span>/);
    expect(out).toContain('data-testid="prop-textAlign"');
    expect(out).toContain('aria-label="Text alignment"');
    for (const a of ['Align left', 'Align center', 'Align right']) expect(out).toContain(`aria-label="${a}"`);
    expect(out).toMatch(/aria-checked="true"[^>]*aria-label="Align center"|aria-label="Align center"[^>]*aria-checked="true"/);
  });
  it('draws no weight and no alignment where the app layer sent none (it sends both for a text, and no default is made here)', () => {
    expect(textStyle(properties({ kind: 'text', sections: TEXT }))).toBe('');
    expect(textStyle(properties({ kind: 'text', sections: TEXT, fontWeight: 700 }))).toBe('');
    expect(textStyle(properties({ kind: 'text', sections: TEXT, textAlign: 'left' }))).toBe('');
  });
});

describe('the React Border section', () => {
  it('shows the width and colour of the border and a position only when there is a border', () => {
    expect(border(properties())).toContain('data-testid="prop-border-width"');
    expect(border(properties())).toContain('data-testid="prop-border-color"');
    expect(border(properties())).not.toContain('aria-label="Border position"');
    const out = border(properties({ border: { width: 2, color: LINE_B, position: 'outside' } }));
    expect(out).toContain('aria-label="Border position"');
    expect(checked(out, 'Border position')).toEqual(['Outside']);
    expect(tag(out, 'input', 'prop-border-width')).toContain('value="2"');
  });
  it('has a Remove border button only while the border width is above 0, named, 24 px and after the position control', () => {
    expect(border(properties())).not.toContain('Remove border');
    expect(border(properties({ border: { width: 0.5, color: LINE_B, position: 'inside' } }))).toContain('data-testid="prop-border-remove"');
    const out = border(properties({ border: { width: 2, color: LINE_B, position: 'outside' } }));
    const button = tag(out, 'button', 'prop-border-remove')!;
    expect(button).toContain('aria-label="Remove border"');
    expect(button).toContain('size-(--mf-size-control-sm)');
    expect(out.indexOf('aria-label="Border position"')).toBeLessThan(out.indexOf('data-testid="prop-border-remove"'));
    expect(out.match(/data-testid="prop-border-remove"/g)).toHaveLength(1);
  });
});

describe('the React Shadow section', () => {
  it('has a row for each shadow with its four numbers, a colour and a remove button, and an add button', () => {
    const out = shadows(properties({ shadows: [shadow, { ...shadow, blur: 20 }] }));
    expect(out.match(/data-testid="prop-shadow"/g)).toHaveLength(2);
    for (const f of ['x', 'y', 'blur', 'spread']) expect(out.match(new RegExp(`data-testid="prop-shadow-${f}"`, 'g'))).toHaveLength(2);
    expect(out).toContain('aria-label="Remove shadow 2"');
    expect(out).toContain('aria-label="Shadow 2 blur"');
    expect(out).toContain('data-testid="prop-shadow-add"');
  });
  it('stops offering to add at the limit the app layer gives, and not before', () => {
    const add = (maxShadows: number) => isDisabled(tag(shadows(properties({ shadows: [shadow, shadow], maxShadows })), 'button', 'prop-shadow-add')!);
    expect(add(2)).toBe(true);
    expect(add(4)).toBe(false);
  });
});

// The two copies draw the same elements with the same classes, names, states and texts, in the same order (a class that is not here
// is a visible difference). What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs, with a tolerance of 0.
describe('the React sections against the Svelte ones', () => {
  const svelte = (component: unknown, props: Properties) => render(TestHost as never, { props: { of: component, props: { properties: props, dispatch } } as never }).body;
  const same = (a: string, b: string) => expect(viewOf(b)).toEqual(viewOf(a));
  const node = properties({ layout: 'row', padding: { top: 4, right: 4, bottom: 4, left: 8 }, notes: [{ note: 'marginIgnored' }], border: { width: 2, color: LINE_B, position: 'outside' }, shadows: [shadow, { ...shadow, blur: 20 }], kind: 'text', sections: { ...TEXT, placement: true }, fontWeight: 700, textAlign: 'right', margin: { top: 8, right: 0, bottom: 0, left: 0 }, absolute: true, sw: 'hug', maxShadows: 2 });
  it('the Layout section', () => same(svelte(LayoutSectionSvelte, node), layout(node)));
  it('the Placement section', () => same(svelte(PlacementSectionSvelte, node), placement(node)));
  it('the Size modes', () => same(svelte(SizeModesSvelte, node), sizes(node)));
  it('the Border section', () => same(svelte(BorderSectionSvelte, node), border(node)));
  it('the Shadow section, with Add shadow disabled at the most', () => same(svelte(ShadowSectionSvelte, node), shadows(node)));
  it('the text style', () => same(svelte(TextStyleSvelte, node), textStyle(node)));
  it('the comparison sees a text, a flag and a state that differ', () => {
    const base = viewOf(shadows(node));
    expect(viewOf(shadows({ ...node, maxShadows: 4 })).tags.map((t) => t.flags)).not.toEqual(base.tags.map((t) => t.flags));
    expect(viewOf(sizes({ ...node, sw: 'fill' })).tags.map((t) => t.attrs)).not.toEqual(viewOf(sizes(node)).tags.map((t) => t.attrs));
    expect(viewOf(sizes({ ...node, notes: [{ note: 'marginIgnored' }] })).texts).not.toEqual(viewOf(sizes({ ...node, notes: [] })).texts);
  });
});
