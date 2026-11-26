import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { ExportDialog } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ExportBody from './ExportBody.svelte';
import { START } from './ui';

const dialog = (over: Partial<ExportDialog> = {}): ExportDialog => ({
  frame: 'f1', name: 'Landing Desktop', format: 'tailwind', html: '<p>hi</p>\n<p>there</p>', css: ':root{}', cssFile: 'meldframe.css',
  htmlBytes: 21, cssBytes: 7, countedBytes: 28, limit: 262144, unsupported: [], warnings: [], assets: [], ...over,
});
const ok = { ok: true as const, file: 'Landing Desktop.png', width: 1440, height: 900 };
const html = (props: Record<string, unknown>) =>
  render(TestHost as never, { props: { of: ExportBody, props: { dialog: dialog(), unreviewed: 0, view: START, onView() {}, scale: 1, onScale() {}, plan: ok, onPng() {}, dispatch() {}, onReview() {}, onClose() {}, ...props } } as never }).body;
const tag = (out: string, id: string) => new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`).exec(out)?.[0] ?? '';
const disabled = (t: string) => /\sdisabled(?![:-])/.test(t);

describe('the HTML tab', () => {
  it('shows the first file with line numbers, its size, and the buttons for it', () => {
    const out = html({});
    expect(out).toContain('data-testid="export-code"');
    expect(out).toContain('&lt;p>hi&lt;/p>');
    expect(out).toContain('21 bytes');
    expect(disabled(tag(out, 'export-copy'))).toBe(false);
    expect(disabled(tag(out, 'export-download'))).toBe(false);
  });

  it('shows the stylesheet, under its own size, when the person chose that file', () => {
    const out = html({ view: { ...START, shown: 'css' } });
    expect(out).toContain(':root{}');
    expect(out).toContain('7 bytes');
  });

  it('has the file switch for the Tailwind form only', () => {
    expect(html({})).toContain('data-testid="export-file"');
    expect(html({ dialog: dialog({ format: 'css', css: '' }) })).not.toContain('data-testid="export-file"');
  });

  it('says the sizes of text are measured as the canvas does and that the opening browser lays the page out, and names the stylesheet by what the app layer gave', () => {
    const out = html({ dialog: dialog({ cssFile: 'site.css' }) });
    expect(out).toMatch(/Text sizes are measured in this browser, with the font the canvas draws/);
    expect(out).toMatch(/laid out by the browser that opens it/);
    expect(out).toMatch(/tab, a link or CJK characters can break differently there \(Warnings names the ones it detects\)/);
    expect(out).not.toMatch(/Text sizes are (estimated|not measured)/);
    expect(out).toContain('site.css');
    expect(out).not.toContain('meldframe.css');
  });

  it('uses the words of the glossary: the HTML, not "the document"', () => {
    expect(html({})).not.toMatch(/\bdocument\b/i);
  });

  it('a frame over the limit gives an alert with both sizes, no code, and neither Copy nor Download', () => {
    const out = html({ dialog: dialog({ error: 'E_EXPORT_TOO_LARGE', html: '', css: '', htmlBytes: 0, cssBytes: 0, countedBytes: 400_000 }) });
    expect(out).toMatch(/role="alert"[^>]*data-testid="export-error"/);
    expect(out).toContain('390.6 KiB');
    expect(out).not.toContain('data-testid="export-code"');
    expect(disabled(tag(out, 'export-copy'))).toBe(true);
    expect(disabled(tag(out, 'export-download'))).toBe(true);
  });

  it('puts the review note above the buttons when batches wait, with a Review button, and none when they do not', () => {
    const out = html({ unreviewed: 2 });
    expect(out).toMatch(/data-testid="export-review"[^>]*>[\s\S]*2 batches are not reviewed yet/);
    expect(out).toMatch(/data-testid="export-review"[\s\S]*<button[^>]*>(?:<!---->)?Review/);
    expect(html({ unreviewed: 0 })).not.toContain('data-testid="export-review"');
  });

  it('shows the review note with Review when the count is unknown, and says unknown, not nothing waiting', () => {
    const out = html({ unreviewed: null });
    expect(out).toMatch(/data-testid="export-review"[^>]*>[\s\S]*not known whether agent work is waiting/);
    expect(out).toMatch(/data-testid="export-review"[\s\S]*<button[^>]*>(?:<!---->)?Review/);
  });

  it('lists what the export could not carry with the node, and says the file is unchanged', () => {
    const out = html({ dialog: dialog({ unsupported: [{ code: 'on_text', node: 't1', nodeName: 'Hero Title', message: 'a text node paints no shadows' }] }) });
    expect(out).toContain('data-testid="export-unsupported"');
    expect(out).toContain('Hero Title');
    expect(out).toContain('on_text');
    expect(out).toContain('Your file is unchanged.');
    expect(html({})).not.toContain('data-testid="export-unsupported"');
  });

  it('draws a node name with markup and with characters that reorder text as plain text', () => {
    const out = html({ dialog: dialog({ unsupported: [{ code: 'c', node: 'n', nodeName: '<img src=x>gnp‮.exe', message: 'm⁦x' }] }) });
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;img src=x>gnp.exe');
    expect(out).not.toMatch(/[‪-‮⁦-⁩]/);
  });
});

describe('the tabs and the PNG tab', () => {
  it('has the HTML and the PNG tab, the chosen one selected, and no PDF', () => {
    const out = html({});
    expect(out).toMatch(/id="export-tab-html"[^>]*aria-selected="true"/);
    expect(out).toMatch(/id="export-tab-png"[^>]*aria-selected="false"/);
    expect(out).not.toMatch(/PDF/);
    expect(html({ view: { ...START, tab: 'png' } })).toMatch(/id="export-tab-png"[^>]*aria-selected="true"/);
  });

  it('shows the file and size of the PNG of the frame it is given, and exports only when there is a plan', () => {
    const png = { ...START, tab: 'png' as const };
    const out = html({ view: png });
    expect(out).toContain('Landing Desktop.png · 1440 × 900 px');
    expect(disabled(tag(out, 'export-png'))).toBe(false);
    const big = html({ view: png, scale: 4, plan: { ok: false, why: 'too_large', asked: { scale: 4, width: 5760, height: 3600 }, fits: { scale: 3, width: 4320, height: 2700 } } });
    expect(disabled(tag(big, 'export-png'))).toBe(true);
    expect(big).toContain('At 4x the image would be 5760 × 3600 px');
    expect(big).toContain('The largest scale that fits is 3x (4320 × 2700 px).');
    expect(html({ view: png, plan: null })).toContain('There is nothing to export.');
  });

  it('offers 0.5x to 4x on the PNG tab', () => {
    const out = html({ view: { ...START, tab: 'png' } });
    const labels = [...out.matchAll(/<button[^>]*role="radio"[^>]*>[\s\S]*?<\/button>/g)].map((m) => />([\d.]+x)</.exec(m[0])?.[1]);
    expect(labels).toEqual(['0.5x', '1x', '2x', '3x', '4x']);
  });

  it('does not say text sizes are estimated on the PNG tab: the canvas draws that with the real text engine', () => {
    expect(html({ view: { ...START, tab: 'png' } })).not.toMatch(/estimated/);
  });
});
