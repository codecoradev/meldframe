import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ExportSection from './ExportSection.svelte';

const html = (props: Record<string, unknown>) => render(TestHost as never, { props: { of: ExportSection, props: { scale: 1, onScale() {}, onPng() {}, onHtml() {}, ...props } } as never }).body;
const tag = (out: string, id: string) => new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`).exec(out)?.[0] ?? '';
const disabled = (t: string) => /\sdisabled(?![:-])/.test(t);
const checkedScale = (out: string) => [...out.matchAll(/<button[^>]*role="radio"[^>]*>[\s\S]*?<\/button>/g)].map((m) => m[0]).filter((b) => b.includes('aria-checked="true"')).map((b) => /(\d)x/.exec(b)?.[1]);
const target = { id: 'f1', name: 'Landing Desktop' };
const ok = { ok: true as const, file: 'Landing Desktop.png', width: 1440, height: 2200 };

describe('the Export section', () => {
  it('is not there when no frame can be exported', () => {
    const out = html({ target: null, plan: null });
    expect(out).not.toContain('data-testid="export-section"');
    expect(out).not.toContain('Export as PNG');
  });

  it('offers the PNG with its file and size, and the HTML dialog, for the selected frame', () => {
    const out = html({ target, plan: ok });
    expect(out).toContain('data-testid="export-section"');
    expect(out).toContain('Landing Desktop.png · 1440 × 2200 px');
    expect(tag(out, 'export-section-png')).not.toBe('');
    expect(disabled(tag(out, 'export-section-png'))).toBe(false);
    expect(out).toContain('HTML + Tailwind…');
    expect(out).toMatch(/aria-label="PNG scale"/);
  });

  it('marks the chosen scale', () => {
    expect(checkedScale(html({ target, plan: ok, scale: 1 }))).toEqual(['1']);
    expect(checkedScale(html({ target, plan: ok, scale: 2 }))).toEqual(['2']);
  });

  it('cannot export a PNG that would be too large, and says why', () => {
    const out = html({ target, plan: { ok: false, why: 'too_large' } });
    expect(disabled(tag(out, 'export-section-png'))).toBe(true);
    expect(out).toContain('larger than 4096 x 4096 pixels in area');
  });

  it('draws the name of the frame nowhere as markup (the plan carries it as text)', () => {
    const out = html({ target, plan: { ok: true, file: '<b>x</b>.png', width: 1, height: 1 } });
    expect(out).not.toContain('<b>x</b>');
  });
});
