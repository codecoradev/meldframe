import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Picker, PickerRow } from '@meldframe/protocol';
import PickerList from './PickerList.svelte';

const row = (over: Partial<PickerRow> = {}): PickerRow => ({ id: 'v1', name: 'color/bg', collection: 'Theme', type: 'color', resolved: '#112233', bound: false, ...over });
const picker = (over: Partial<Picker> = {}): Picker => ({ id: 'n', field: 'fill', type: 'color', query: '', total: 2, rows: [row(), row({ id: 'v2', name: 'color/fg', resolved: '#ffffff', bound: true })], ...over });
const html = (p: Picker) => render(PickerList as never, { props: { picker: p, dispatch: () => {} } as never }).body;

describe('the content of the bind picker as the Inspector board draws it', () => {
  it('has a search holding the query and a list named for the field', () => {
    const out = html(picker({ query: 'bg' }));
    expect(out).toContain('aria-label="Search variables"');
    expect(out).toMatch(/data-testid="picker-search"[^>]*value="bg"|value="bg"[^>]*data-testid="picker-search"/);
    expect(out).toContain('aria-label="Variables for Fill"');
  });
  it('has a row for each variable with its name, its value and, for a colour, a swatch of the value', () => {
    const out = html(picker());
    expect(out.match(/data-testid="picker-row"/g)).toHaveLength(2);
    expect(out).toContain('color/bg');
    expect(out).toContain('#ffffff');
    expect(out).toContain('background: #112233');
  });
  it('shows the row the field is bound to as pressed, and the others as not', () => {
    const out = html(picker());
    expect(out.match(/aria-pressed="true"/g)).toHaveLength(1);
    expect(out.match(/aria-pressed="false"/g)).toHaveLength(1);
    expect(out).toMatch(/aria-pressed="true"[^>]*data-id="v2"/);
  });
  it('draws no swatch for a number', () => {
    const out = html(picker({ field: 'opacity', type: 'number', rows: [row({ type: 'number', resolved: '0.5' })] }));
    expect(out).not.toContain('background:');
    expect(out).toContain('aria-label="Variables for Opacity"');
    expect(out).toContain('Only number variables are shown for Opacity.');
  });
  it('says there are no variables of the type, or none that match the search, and only then', () => {
    expect(html(picker({ total: 0, rows: [] }))).toMatch(/data-testid="picker-empty"[^>]*>This file has no color variables\./);
    expect(html(picker({ total: 3, rows: [], query: 'zz' }))).toMatch(/data-testid="picker-empty"[^>]*>No variable matches the search\./);
    expect(html(picker())).not.toContain('picker-empty');
  });
});
