// The React copies of IconButton, Tooltip, Menu and the state matrix, checked as chrome.test.ts and forms.test.ts check
// the Svelte ones: the markup of a server render, in Node. A popup is not in a server render (it opens on a pointer or a
// key), so what it does is the job of packages/web-chrome/scripts/overlays-e2e.mjs, which asks the Svelte and the React
// fixture the same questions in a browser.
import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { square } from '../focus';
import { IconButton } from './IconButton';
import { Menu } from './Menu';
import { StateMatrix } from './StateMatrix';
import { TOOLTIP_DELAY_MS, TOOLTIP_SKIP_MS, TooltipProvider } from './Tooltip';

// What the Layout gives the real controls: one provider above them.
const html = (el: ReactElement) => renderToStaticMarkup(<TooltipProvider>{el}</TooltipProvider>);

describe('labels are required', () => {
  it('an IconButton without a name does not render', () => {
    expect(() => html(<IconButton label="" icon="plus" />)).toThrow(/needs a label/);
  });
});

describe('IconButton', () => {
  it('is a button named by its label, with an icon that is hidden from the tree', () => {
    const out = html(<IconButton label="Layers panel" icon="panel-left" />);
    expect(out).toContain('aria-label="Layers panel"');
    expect(out).toContain('type="button"');
    expect(out).toContain('aria-hidden="true"');
  });
  it('is a toggle only when `pressed` is given', () => {
    expect(html(<IconButton label="A" icon="plus" />)).not.toContain('aria-pressed');
    expect(html(<IconButton label="A" icon="plus" pressed />)).toContain('aria-pressed="true"');
    expect(html(<IconButton label="A" icon="plus" pressed={false} />)).toContain('aria-pressed="false"');
  });
  it('draws a pressed toggle with a fill and an outline, not by fill alone (about 1.3:1 against the card)', () => {
    expect(html(<IconButton label="A" icon="plus" pressed />)).toMatch(/class="[^"]*border-border-strong[^"]*bg-control/);
    expect(html(<IconButton label="A" icon="plus" pressed={false} />)).toMatch(/class="[^"]*border-transparent/);
  });
  it('can be disabled, and takes its size from the size tokens', () => {
    expect(html(<IconButton label="A" icon="plus" disabled />)).toMatch(/<button[^>]*disabled/);
    expect(html(<IconButton label="A" icon="plus" size={24} />)).toContain('size-(--mf-size-control-sm)');
    expect(Object.keys(square)).toEqual(['24', '28', '32', '36']);
  });
  it('passes the test id and the handlers a wrapping trigger hands it on to its button', () => {
    expect(html(<IconButton label="A" icon="plus" data-testid="tool-plus" />)).toMatch(/<button[^>]*data-testid="tool-plus"/);
  });
  it('shows no tooltip until a pointer or a key asks for it', () => {
    const out = html(<IconButton label="Layers panel" icon="panel-left" shortcut="Alt+1" />);
    expect(out).not.toContain('role="tooltip"');
    expect(out).not.toContain('Alt+1');
  });
});

describe('the tooltip provider', () => {
  it('waits 400 ms for the first tooltip of a group and lets its neighbours open at once within 300 ms', () => {
    expect([TOOLTIP_DELAY_MS, TOOLTIP_SKIP_MS]).toEqual([400, 300]);
  });
});

describe('Menu', () => {
  const items = [
    { id: 'a', label: 'Alpha', onSelect: () => {}, shortcut: 'A' },
    { id: 'b', label: 'Bravo', onSelect: () => {}, disabled: true },
  ];
  it('closed, is its trigger alone: an IconButton named by the label, that says it opens a menu', () => {
    const out = html(<Menu label="File menu" icon="menu" items={items} testId="file-menu" />);
    expect(out).toContain('aria-label="File menu"');
    expect(out).toContain('data-testid="file-menu"');
    expect(out).toContain('aria-haspopup="menu"');
    expect(out).toContain('aria-expanded="false"');
    expect(out).not.toContain('role="menu"');
    expect(out).not.toContain('Alpha');
  });
  it('with `text` the trigger is a plain text button that shows it and keeps the label as its name', () => {
    const out = html(<Menu label="Zoom 100%" text="100%" items={items} testId="zoom" />);
    expect(out).toMatch(/<button[^>]*aria-label="Zoom 100%"[^>]*>100%<\/button>/);
    expect(out).toContain('font-mono');
    expect(out).toContain('data-testid="zoom"');
  });
  it('refuses a trigger without a name', () => {
    expect(() => html(<Menu label="" items={items} />)).toThrow(/needs a label/);
  });
});

describe('the state matrix page', () => {
  const out = renderToStaticMarkup(<StateMatrix />);
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
  it('has the controls the Svelte page has: 47 buttons and inputs in each theme', () => {
    const themes = out.split('<section data-theme=').slice(1);
    expect(themes).toHaveLength(2);
    for (const part of themes) expect(part.match(/<(?:button|input)[\s>]/g)).toHaveLength(47);
  });
});
