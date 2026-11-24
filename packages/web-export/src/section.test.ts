import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ExportSection from './ExportSection.svelte';

const html = (props: Record<string, unknown>) => render(TestHost as never, { props: { of: ExportSection, props: { scale: 1, onScale() {}, onPng() {}, onHtml() {}, ...props } } as never }).body;
const tag = (out: string, id: string) => new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`).exec(out)?.[0] ?? '';
const disabled = (t: string) => /\sdisabled(?![:-])/.test(t);
const radios = (out: string) => [...out.matchAll(/<button[^>]*role="radio"[^>]*>[\s\S]*?<\/button>/g)].map((m) => m[0]);
const checkedScale = (out: string) => radios(out).filter((b) => b.includes('aria-checked="true"')).map((b) => /([\d.]+)x/.exec(b)?.[1]);
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

  it('offers 0.5x, 1x, 2x, 3x and 4x', () => {
    const labels = radios(html({ target, plan: ok })).map((b) => />([\d.]+x)</.exec(b)?.[1]);
    expect(labels).toEqual(['0.5x', '1x', '2x', '3x', '4x']);
  });

  it('marks the chosen scale', () => {
    for (const s of [0.5, 1, 2, 3, 4]) expect(checkedScale(html({ target, plan: ok, scale: s }))).toEqual([String(s)]);
  });

  it('cannot export a PNG that would be too large, and says why and which scale fits', () => {
    const plan = { ok: false, why: 'too_large', asked: { scale: 4, width: 5760, height: 3600 }, fits: { scale: 3, width: 4320, height: 2700 } };
    const out = html({ target, plan, scale: 4 });
    expect(disabled(tag(out, 'export-section-png'))).toBe(true);
    expect(out).toContain('At 4x the image would be 5760 × 3600 px, more than the limit of 4096 × 4096 px in area.');
    expect(out).toContain('The largest scale that fits is 3x (4320 × 2700 px).');
  });

  it('draws the name of the frame nowhere as markup (the plan carries it as text)', () => {
    const out = html({ target, plan: { ok: true, file: '<b>x</b>.png', width: 1, height: 1 } });
    expect(out).not.toContain('<b>x</b>');
  });
});
