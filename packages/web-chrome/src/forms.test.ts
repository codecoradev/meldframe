import { render } from 'svelte/server';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import Checkbox from './Checkbox.svelte';
import Field from './Field.svelte';
import Section from './Section.svelte';
import Segmented from './Segmented.svelte';
import StateMatrix from './StateMatrix.svelte';
import Tabs from './Tabs.svelte';
import { fieldHeight } from './focus';

const html = (c: unknown, props: Record<string, unknown>) => render(c as never, { props: props as never }).body;
const text = (s: string) => createRawSnippet(() => ({ render: () => `<span>${s}</span>` }));

describe('names are required', () => {
  it('a Field, Segmented and Tabs without a name do not render', () => {
    expect(() => html(Field, { label: '', value: '1' })).toThrow(/needs a label/);
    expect(() => html(Segmented, { label: '', options: [], value: '', onChange: () => {} })).toThrow(/needs a label/);
    expect(() => html(Tabs, { label: '', tabs: [], selected: '', onSelect: () => {}, idPrefix: 'x' })).toThrow(/needs a label/);
  });
});

describe('Field placeholder', () => {
  it('shows a placeholder on the input, and none unless one is given', () => {
    expect(html(Field, { label: 'Pad', value: '', placeholder: 'Mixed' })).toContain('placeholder="Mixed"');
    expect(html(Field, { label: 'Pad', value: '' })).not.toContain('placeholder');
  });
});

describe('Segmented', () => {
  const options = [{ value: 'row', label: 'Row' }, { value: 'column', label: 'Column' }, { value: 'grid', label: 'Grid' }];
  it('is a radio group: one checked option, the checked one alone in the Tab order', () => {
    const out = html(Segmented, { label: 'Flow', options, value: 'column', onChange: () => {} });
    expect(out).toContain('role="radiogroup"');
    expect(out).toContain('aria-label="Flow"');
    expect(out.match(/aria-checked="true"/g)).toHaveLength(1);
    expect(out.match(/tabindex="0"/g)).toHaveLength(1);
    expect(out.match(/tabindex="-1"/g)).toHaveLength(2);
  });
  it('names an icon-only option by its label', () => {
    const out = html(Segmented, { label: 'Flow', options: options.map((o) => ({ ...o, icon: 'flow-row' })), value: 'row', iconOnly: true, onChange: () => {} });
    expect(out).toContain('aria-label="Row"');
    expect(out).not.toContain('>Row<');
  });
  it('disables every option', () => {
    expect(html(Segmented, { label: 'Flow', options, value: 'row', disabled: true, onChange: () => {} }).match(/ disabled/g)).toHaveLength(3);
  });
});

describe('Tabs', () => {
  const tabs = [{ id: 'design', label: 'Design' }, { id: 'activity', label: 'Activity', badge: 3 }];
  it('is a tab list with one selected tab wired to its panel', () => {
    const out = html(Tabs, { label: 'Right panel', tabs, selected: 'activity', onSelect: () => {}, idPrefix: 'rp' });
    expect(out).toContain('role="tablist"');
    expect(out.match(/role="tab"/g)).toHaveLength(2);
    expect(out).toContain('aria-controls="rp-panel-activity"');
    expect(out).toContain('id="rp-tab-design"');
    expect(out.match(/aria-selected="true"/g)).toHaveLength(1);
    expect(out).toContain('>3<');
  });
  it('says what a badge counts to a reader, who does not see the colour', () => {
    const unknown = [{ id: 'design', label: 'Design' }, { id: 'activity', label: 'Activity', badge: null, badgeLabel: 'batches to review' }];
    const shown = html(Tabs, { label: 'Right panel', tabs: unknown, selected: 'design', onSelect: () => {}, idPrefix: 'rp' }).replace(/<!--.*?-->/g, '');
    expect(shown).toMatch(/>\?<span class="sr-only"> batches to review: unknown<\/span>/);
    const named = [{ id: 'design', label: 'Design' }, { id: 'activity', label: 'Activity', badge: 3, badgeLabel: 'batches to review' }];
    const out = html(Tabs, { label: 'Right panel', tabs: named, selected: 'design', onSelect: () => {}, idPrefix: 'rp' });
    expect(out.replace(/<!--.*?-->/g, '')).toMatch(/>3<span class="sr-only"> batches to review<\/span>/);
    expect(html(Tabs, { label: 'Right panel', tabs, selected: 'design', onSelect: () => {}, idPrefix: 'rp' })).not.toContain('sr-only');
    // a count is not an agent: the badge does not wear an agent colour
    expect(out).not.toContain('bg-agent-1');
    expect(out).toContain('bg-accent-strong');
  });
});
describe('Segmented in light mode', () => {
  it('outlines the selected cell: control-selected on the track is about 1.1:1 by fill alone', () => {
    const out = html(Segmented, { label: 'Panel', options: [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }], value: 'a', onChange: () => {} });
    expect(out).toMatch(/aria-checked="true"[^>]*class="[^"]*border-border-strong[^"]*bg-control-selected/);
    expect(out).toMatch(/aria-checked="false"[^>]*class="[^"]*border-transparent/);
  });
});

describe('a disabled tab', () => {
  const tabs = [{ id: 'design', label: 'Design' }, { id: 'activity', label: 'Activity', disabled: true }];
  const out = html(Tabs, { label: 'Right panel', tabs, selected: 'design', onSelect: () => {}, idPrefix: 'right' });
  it('is marked, has no panel to control, and the selected tab keeps the Tab stop', () => {
    expect(out).toMatch(/id="right-tab-activity"[^>]*aria-disabled="true"/);
    expect(out).not.toMatch(/id="right-tab-activity"[^>]*aria-controls/);
    expect(out).toMatch(/id="right-tab-design"[^>]*aria-controls="right-panel-design"/);
    expect(out).toMatch(/id="right-tab-design"[^>]*tabindex="0"/);
  });
});

describe('Field and Section', () => {
  it('a Field names its input, and marks an invalid one', () => {
    const out = html(Field, { label: 'Width', prefix: 'W', value: '240', invalid: true });
    expect(out).toContain('aria-label="Width"');
    expect(out).toContain('aria-invalid="true"');
    expect(html(Field, { label: 'Width', value: '1' })).not.toContain('aria-invalid');
    expect(html(Field, { label: 'Width', value: '1', disabled: true })).toMatch(/<input[^>]*disabled/);
  });
  it('a Field puts the test id on its input and draws what leads it', () => {
    const out = html(Field, { label: 'Fill', value: '#fff', testId: 'prop-fill', lead: createRawSnippet(() => ({ render: () => '<i data-swatch></i>' })) });
    expect(out).toMatch(/<input[^>]*data-testid="prop-fill"/);
    expect(out.indexOf('data-swatch')).toBeLessThan(out.indexOf('<input'));
  });
  it('a Checkbox is a named native checkbox, drawn filled with a check when on', () => {
    const on = html(Checkbox, { label: 'Clip content', checked: true, onChange: () => {}, 'data-testid': 'prop-clip' });
    expect(on).toMatch(/<input[^>]*type="checkbox"[^>]*checked/);
    expect(on).toContain('Clip content');
    expect(on).toContain('bg-accent-strong');
    expect(html(Checkbox, { label: 'Clip content', checked: false, onChange: () => {} })).not.toContain('bg-accent-strong');
    expect(() => html(Checkbox, { label: ' ', checked: false, onChange: () => {} })).toThrow(/needs a label/);
  });
  it('field heights are 28, 32 and 34 from the size tokens', () => {
    expect(fieldHeight).toEqual({ dense: 'h-(--mf-size-control-md)', default: 'h-(--mf-size-control-lg)', wide: 'h-(--mf-size-control-wide)' });
  });
  it('a Section folds only when it has a toggle, and says so', () => {
    const body = text('INNER');
    expect(html(Section, { title: 'Fill', children: body })).not.toContain('aria-expanded');
    const open = html(Section, { title: 'Fill', open: true, onToggle: () => {}, children: body });
    expect(open).toContain('aria-expanded="true"');
    expect(open).toContain('INNER');
    const shut = html(Section, { title: 'Fill', open: false, onToggle: () => {}, children: body });
    expect(shut).toContain('aria-expanded="false"');
    expect(shut).not.toContain('INNER');
  });
});


describe('the state matrix page', () => {
  const out = render(StateMatrix as never, { props: {} }).body;
  it('draws both themes with the six states', () => {
    expect(out).toContain('data-theme="dark"');
    expect(out).toContain('data-theme="light"');
    for (const s of ['default', 'hover', 'pressed', 'focus', 'disabled', 'selected']) expect(out).toContain(`>${s}<`);
    for (const s of ['hover', 'pressed', 'focus']) expect(out).toContain(`data-preview="${s}"`);
  });
  it('gives every icon-only button a name, and no id twice', () => {
    const buttons = out.match(/<button[\s\S]*?<\/button>/g) ?? [];
    expect(buttons.length).toBeGreaterThan(20);
    for (const b of buttons) if (b.replace(/<[^>]*>/g, '').trim() === '') expect(b).toContain('aria-label=');
    const ids = out.match(/ id="[^"]+"/g) ?? [];
    expect(new Set(ids).size).toBe(ids.length);
  });
});
describe('hit areas', () => {
  it('the Section toggle is at least 24 px high', () => {
    expect(html(Section, { title: 'Fill', open: true, onToggle: () => {}, children: text('x') })).toContain('h-(--mf-size-control-sm)');
  });
});
