// The React copies of the chrome primitives, checked as forms.test.ts and chrome.test.ts check the Svelte ones: the same
// assertions on the markup of a server render (Node, no DOM library). What a browser alone can prove (focus, keys,
// pixels) is left to packages/web-chrome/scripts/states-e2e.mjs, run against both versions.
import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { fieldHeight } from '../focus';
import { icons } from '../icons';
import { Button } from './Button';
import { Checkbox } from './Checkbox';
import { Field } from './Field';
import { Icon } from './Icon';
import { Section } from './Section';
import { Segmented } from './Segmented';
import { Tabs } from './Tabs';

const html = (el: ReactElement) => renderToStaticMarkup(el);
const noop = () => {};

describe('names are required', () => {
  it('a Field, Segmented, Tabs and Checkbox without a name do not render', () => {
    expect(() => html(<Field label="" value="1" />)).toThrow(/needs a label/);
    expect(() => html(<Segmented label="" options={[]} value="" onChange={noop} />)).toThrow(/needs a label/);
    expect(() => html(<Tabs label="" tabs={[]} selected="" onSelect={noop} idPrefix="x" />)).toThrow(/needs a label/);
    expect(() => html(<Checkbox label=" " checked={false} onChange={noop} />)).toThrow(/needs a label/);
  });
});

describe('Icon', () => {
  it('draws its path on a 16 px grid, hidden from the tree, with the path data of the Svelte icon', () => {
    const out = html(<Icon name="plus" />);
    expect(out).toContain('viewBox="0 0 16 16"');
    expect(out).toContain('aria-hidden="true"');
    expect(out).toContain(`d="${icons.plus}"`);
    expect(out).toContain('size-4');
  });
});

describe('Button', () => {
  it('is a button that can be disabled, and the strong fills carry the text', () => {
    const out = html(
      <Button variant="primary" disabled>
        Save
      </Button>,
    );
    expect(out).toMatch(/<button[^>]*\sdisabled[=\s>]/);
    expect(out).toContain('type="button"');
    expect(out).toContain('bg-accent-strong');
    expect(out).not.toContain('bg-accent ');
    expect(html(<Button variant="destructive">Revoke</Button>)).toContain('bg-danger-strong');
    expect(html(<Button>Cancel</Button>)).toContain('bg-control ');
  });
  it('is a toggle only when `pressed` is given, drawn in the chip colours when on', () => {
    expect(html(<Button>A</Button>)).not.toContain('aria-pressed');
    const on = html(<Button pressed>A</Button>);
    expect(on).toContain('aria-pressed="true"');
    expect(on).toContain('bg-chip-bg');
    expect(html(<Button pressed={false}>A</Button>)).toContain('aria-pressed="false"');
  });
  it('is 32 high, or 28 for a row of a panel, and passes a name, a description and a test id on', () => {
    expect(html(<Button>A</Button>)).toContain('h-(--mf-size-control-lg)');
    expect(html(<Button size="sm">A</Button>)).toContain('h-(--mf-size-control-md)');
    const out = html(
      <Button aria-label="Save design" aria-describedby="why" data-testid="save">
        Save
      </Button>,
    );
    expect(out).toContain('aria-label="Save design"');
    expect(out).toContain('aria-describedby="why"');
    expect(out).toContain('data-testid="save"');
  });
});

describe('Field placeholder', () => {
  it('shows a placeholder on the input, and none unless one is given', () => {
    expect(html(<Field label="Pad" value="" placeholder="Mixed" />)).toContain('placeholder="Mixed"');
    expect(html(<Field label="Pad" value="" />)).not.toContain('placeholder');
  });
});

describe('Segmented', () => {
  const options = [
    { value: 'row', label: 'Row' },
    { value: 'column', label: 'Column' },
    { value: 'grid', label: 'Grid' },
  ];
  it('is a radio group: one checked option, the checked one alone in the Tab order', () => {
    const out = html(<Segmented label="Flow" options={options} value="column" onChange={noop} />);
    expect(out).toContain('role="radiogroup"');
    expect(out).toContain('aria-label="Flow"');
    expect(out.match(/aria-checked="true"/g)).toHaveLength(1);
    expect(out.match(/tabindex="0"/g)).toHaveLength(1);
    expect(out.match(/tabindex="-1"/g)).toHaveLength(2);
  });
  it('names an icon-only option by its label', () => {
    const out = html(<Segmented label="Flow" options={options.map((o) => ({ ...o, icon: 'flow-row' as const }))} value="row" iconOnly onChange={noop} />);
    expect(out).toContain('aria-label="Row"');
    expect(out).not.toContain('>Row<');
  });
  it('disables every option', () => {
    expect(html(<Segmented label="Flow" options={options} value="row" disabled onChange={noop} />).match(/ disabled/g)).toHaveLength(3);
  });
  it('outlines the selected cell: control-selected on the track is about 1.1:1 by fill alone', () => {
    const out = html(
      <Segmented
        label="Panel"
        options={[
          { value: 'a', label: 'A' },
          { value: 'b', label: 'B' },
        ]}
        value="a"
        onChange={noop}
      />,
    );
    expect(out).toMatch(/aria-checked="true"[^>]*class="[^"]*border-border-strong[^"]*bg-control-selected/);
    expect(out).toMatch(/aria-checked="false"[^>]*class="[^"]*border-transparent/);
  });
});

describe('Tabs', () => {
  const tabs = [
    { id: 'design', label: 'Design' },
    { id: 'activity', label: 'Activity', badge: 3 },
  ];
  it('is a tab list with one selected tab wired to its panel', () => {
    const out = html(<Tabs label="Right panel" tabs={tabs} selected="activity" onSelect={noop} idPrefix="rp" />);
    expect(out).toContain('role="tablist"');
    expect(out.match(/role="tab"/g)).toHaveLength(2);
    expect(out).toContain('aria-controls="rp-panel-activity"');
    expect(out).toContain('id="rp-tab-design"');
    expect(out.match(/aria-selected="true"/g)).toHaveLength(1);
    expect(out).toContain('>3<');
  });
  it('draws a count that is not known as ? and says unknown to a reader, and draws nothing for no badge', () => {
    const unknown = [tabs[0]!, { id: 'activity', label: 'Activity', badge: null, badgeLabel: 'batches to review' }];
    const out = html(<Tabs label="Right panel" tabs={unknown} selected="design" onSelect={noop} idPrefix="rp" />);
    expect(out).toMatch(/>\?<span class="sr-only"> batches to review: unknown<\/span>/);
    expect(out).toContain('data-unknown="true"');
    expect(html(<Tabs label="Right panel" tabs={[tabs[0]!, { id: 'activity', label: 'Activity', badge: 0 }]} selected="design" onSelect={noop} idPrefix="rp" />)).not.toContain('?');
  });
  it('says what a badge counts to a reader, who does not see the colour', () => {
    const named = [tabs[0]!, { id: 'activity', label: 'Activity', badge: 3, badgeLabel: 'batches to review' }];
    const out = html(<Tabs label="Right panel" tabs={named} selected="design" onSelect={noop} idPrefix="rp" />);
    expect(out).toMatch(/>3<span class="sr-only"> batches to review<\/span>/);
    expect(html(<Tabs label="Right panel" tabs={tabs} selected="design" onSelect={noop} idPrefix="rp" />)).not.toContain('sr-only');
    // a count is not an agent: the badge does not wear an agent colour
    expect(out).not.toContain('bg-agent-1');
    expect(out).toContain('bg-accent-strong');
  });
  describe('a disabled tab', () => {
    const off = [tabs[0]!, { id: 'activity', label: 'Activity', disabled: true }];
    const out = html(<Tabs label="Right panel" tabs={off} selected="design" onSelect={noop} idPrefix="right" />);
    it('is marked, has no panel to control, and the selected tab keeps the Tab stop', () => {
      expect(out).toMatch(/id="right-tab-activity"[^>]*aria-disabled="true"/);
      expect(out).not.toMatch(/id="right-tab-activity"[^>]*aria-controls/);
      expect(out).toMatch(/id="right-tab-design"[^>]*aria-controls="right-panel-design"/);
      expect(out).toMatch(/id="right-tab-design"[^>]*tabindex="0"/);
    });
  });
});

describe('Field and Section', () => {
  it('a Field names its input, and marks an invalid one', () => {
    const out = html(<Field label="Width" prefix="W" value="240" invalid />);
    expect(out).toContain('aria-label="Width"');
    expect(out).toContain('aria-invalid="true"');
    expect(out).toContain('value="240"');
    expect(html(<Field label="Width" value="1" />)).not.toContain('aria-invalid');
    expect(html(<Field label="Width" value="1" disabled />)).toMatch(/<input[^>]*disabled/);
  });
  it('a Field puts the test id on its input and draws what leads it', () => {
    const out = html(<Field label="Fill" value="accent/base" testId="prop-fill" lead={<i data-swatch="" />} />);
    expect(out).toMatch(/<input[^>]*data-testid="prop-fill"/);
    expect(out.indexOf('data-swatch')).toBeLessThan(out.indexOf('<input'));
  });
  it('a Field looks bound in the chip colours, and mono in the mono face', () => {
    expect(html(<Field label="Spacing" value="space/md" bound />)).toContain('bg-chip-bg');
    expect(html(<Field label="Spacing" value="space/md" mono />)).toContain('font-mono');
  });
  it('a Checkbox is a named native checkbox, drawn filled with a check when on', () => {
    const on = html(<Checkbox label="Clip content" checked onChange={noop} data-testid="prop-clip" />);
    expect(on).toMatch(/<input[^>]*type="checkbox"[^>]*checked/);
    expect(on).toMatch(/<input[^>]*data-testid="prop-clip"/);
    expect(on).toContain('Clip content');
    expect(on).toContain('bg-accent-strong');
    expect(on).toContain('<svg');
    const off = html(<Checkbox label="Clip content" checked={false} onChange={noop} />);
    expect(off).not.toContain('bg-accent-strong');
    expect(off).not.toContain('<svg');
  });
  it('field heights are 28, 32 and 34 from the size tokens', () => {
    expect(fieldHeight).toEqual({ dense: 'h-(--mf-size-control-md)', default: 'h-(--mf-size-control-lg)', wide: 'h-(--mf-size-control-wide)' });
    expect(html(<Field label="A" value="" size="dense" />)).toContain('h-(--mf-size-control-md)');
    expect(html(<Field label="A" value="" size="wide" />)).toContain('h-(--mf-size-control-wide)');
  });
  it('a Section folds only when it has a toggle, and says so', () => {
    const body = <span>INNER</span>;
    expect(html(<Section title="Fill">{body}</Section>)).not.toContain('aria-expanded');
    const open = html(
      <Section title="Fill" open onToggle={noop}>
        {body}
      </Section>,
    );
    expect(open).toContain('aria-expanded="true"');
    expect(open).toContain('INNER');
    const shut = html(
      <Section title="Fill" open={false} onToggle={noop}>
        {body}
      </Section>,
    );
    expect(shut).toContain('aria-expanded="false"');
    expect(shut).not.toContain('INNER');
  });
  it('a Section names itself, puts the test id on its toggle and shows its actions', () => {
    const out = html(
      <Section title="Fill" onToggle={noop} toggleId="fill-toggle" actions={<i data-action="" />}>
        x
      </Section>,
    );
    expect(out).toContain('aria-label="Fill"');
    expect(out).toMatch(/<button[^>]*data-testid="fill-toggle"/);
    expect(out).toContain('data-action');
  });
});

describe('hit areas', () => {
  it('the Section toggle is at least 24 px high', () => {
    expect(
      html(
        <Section title="Fill" open onToggle={noop}>
          x
        </Section>,
      ),
    ).toContain('h-(--mf-size-control-sm)');
  });
});
