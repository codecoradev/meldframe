import { render } from 'svelte/server';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import Button from './Button.svelte';
import IconButton from './IconButton.svelte';
import { icons } from './Icon.svelte';
import TestHost from './TestHost.svelte';
import { nextIndex, requireLabel, square } from './focus';

const html = (c: unknown, props: Record<string, unknown>) => render(TestHost as never, { props: { of: c, props } as never }).body;
const text = (s: string) => createRawSnippet(() => ({ render: () => `<span>${s}</span>` }));

describe('the keyboard rule of a roving group', () => {
  it('moves with the arrows, wraps, and jumps with Home and End', () => {
    expect(nextIndex('ArrowRight', 0, 3)).toBe(1);
    expect(nextIndex('ArrowRight', 2, 3)).toBe(0);
    expect(nextIndex('ArrowLeft', 0, 3)).toBe(2);
    expect(nextIndex('Home', 2, 3)).toBe(0);
    expect(nextIndex('End', 0, 3)).toBe(2);
  });
  it('skips a disabled member with the arrows, Home and End, and does nothing when none is enabled', () => {
    const off = [false, true];
    expect(nextIndex('ArrowRight', 0, 2, off)).toBe(0);
    expect(nextIndex('ArrowLeft', 0, 2, off)).toBe(0);
    expect(nextIndex('End', 0, 2, off)).toBe(0);
    expect(nextIndex('ArrowRight', 0, 3, [false, true, false])).toBe(2);
    expect(nextIndex('Home', 2, 3, [true, false, false])).toBe(1);
    expect(nextIndex('ArrowRight', 0, 2, [true, true])).toBeUndefined();
  });
  it('ignores every other key and an empty group', () => {
    expect(nextIndex('a', 1, 3)).toBeUndefined();
    expect(nextIndex('Enter', 1, 3)).toBeUndefined();
    expect(nextIndex('ArrowRight', 0, 0)).toBeUndefined();
  });
});

describe('labels are required', () => {
  it('refuses an empty or blank label', () => {
    expect(() => requireLabel('', 'IconButton')).toThrow(/needs a label/);
    expect(() => requireLabel('   ', 'Field')).toThrow(/needs a label/);
    expect(() => requireLabel(undefined, 'Tabs')).toThrow(/needs a label/);
    expect(requireLabel('Layers', 'IconButton')).toBe('Layers');
  });
  it('an IconButton without a name does not render', () => {
    expect(() => html(IconButton, { label: '', icon: 'plus' })).toThrow(/needs a label/);
  });
});

describe('IconButton', () => {
  it('is a button named by its label, with an icon that is hidden from the tree', () => {
    const out = html(IconButton, { label: 'Layers panel', icon: 'panel-left' });
    expect(out).toContain('aria-label="Layers panel"');
    expect(out).toContain('type="button"');
    expect(out).toContain('aria-hidden="true"');
  });
  it('is a toggle only when `pressed` is given', () => {
    expect(html(IconButton, { label: 'A', icon: 'plus' })).not.toContain('aria-pressed');
    expect(html(IconButton, { label: 'A', icon: 'plus', pressed: true })).toContain('aria-pressed="true"');
    expect(html(IconButton, { label: 'A', icon: 'plus', pressed: false })).toContain('aria-pressed="false"');
  });
  it('draws a pressed toggle with a fill and an outline, not by fill alone (about 1.3:1 against the card)', () => {
    expect(html(IconButton, { label: 'A', icon: 'plus', pressed: true })).toMatch(/class="[^"]*border-border-strong[^"]*bg-control/);
    expect(html(IconButton, { label: 'A', icon: 'plus', pressed: false })).toMatch(/class="[^"]*border-transparent/);
  });
  it('can be disabled, and takes its size from the size tokens', () => {
    expect(html(IconButton, { label: 'A', icon: 'plus', disabled: true })).toMatch(/<button[^>]*disabled/);
    expect(html(IconButton, { label: 'A', icon: 'plus', size: 24 })).toContain('size-(--mf-size-control-sm)');
    expect(Object.keys(square)).toEqual(['24', '28', '32', '36']);
  });
  it('draws only icons that exist', () => {
    expect(Object.keys(icons).length).toBeGreaterThan(25);
    for (const d of Object.values(icons)) expect(d).toMatch(/^M[\d. ]/);
  });
});

describe('Button', () => {
  it('is a button that can be disabled', () => {
    const out = html(Button, { variant: 'primary', disabled: true, children: text('Save') });
    expect(out).toMatch(/<button[^>]*\sdisabled[=\s>]/);
    expect(out).toContain('bg-accent-strong');
    expect(out).not.toContain('bg-accent ');
  });
});
