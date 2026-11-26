// The React copies of the Export section and of the dialog's body, checked as body.test.ts and section.test.ts check the
// Svelte ones: the markup of a server render, in Node. The shell of the dialog (the portal, the focus, Escape, the
// scrim) is not in a server render: it is the job of packages/web-export/scripts/export-e2e.mjs (MELDFRAME_UI=react), which
// asks the Svelte and the React page the same questions in a browser. The rules (png, save, ui, view)
// are shared with the Svelte copy and keep their own tests.
import type { ExportDialog as Model } from '@meldframe/protocol';
import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { START } from '../ui';
import { ExportBody, type ExportBodyProps } from './ExportBody';
import { ExportDialog } from './ExportDialog';
import { ExportSection, type ExportSectionProps } from './ExportSection';

const dialog = (over: Partial<Model> = {}): Model => ({
  frame: 'f1', name: 'Landing Desktop', format: 'tailwind', html: '<p>hi</p>\n<p>there</p>', css: ':root{}', cssFile: 'meldframe.css',
  htmlBytes: 21, cssBytes: 7, countedBytes: 28, limit: 262144, unsupported: [], warnings: [], assets: [], ...over,
});
const ok = { ok: true as const, file: 'Landing Desktop.png', width: 1440, height: 900 };
const draw = (el: ReactElement) => renderToStaticMarkup(el);
const body = (props: Partial<ExportBodyProps>) =>
  draw(<ExportBody dialog={dialog()} unreviewed={0} view={START} onView={() => {}} scale={1} onScale={() => {}} plan={ok} onPng={() => {}} dispatch={() => {}} onReview={() => {}} onClose={() => {}} {...props} />);
const tag = (out: string, id: string) => new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`).exec(out)?.[0] ?? '';
const disabled = (t: string) => /\sdisabled(?![:-])/.test(t);
const radios = (out: string) => [...out.matchAll(/<button[^>]*role="radio"[^>]*>[\s\S]*?<\/button>/g)].map((m) => m[0]);
const scales = (out: string) => radios(out).map((b) => />([\d.]+x)</.exec(b)?.[1]);

describe('the HTML tab', () => {
  it('shows the first file with line numbers, its size, and the buttons for it', () => {
    const out = body({});
    expect(out).toContain('data-testid="export-code"');
    expect(out).toContain('&lt;p&gt;hi&lt;/p&gt;');
    expect(out).toContain('21 bytes');
    expect(disabled(tag(out, 'export-copy'))).toBe(false);
    expect(disabled(tag(out, 'export-download'))).toBe(false);
  });

  it('draws the preview as a keyboard-reachable region named by its file', () => {
    expect(body({})).toMatch(/data-testid="export-code"[^>]*tabindex="0"[^>]*role="region"[^>]*aria-label="Landing Desktop\.html preview"|aria-label="[^"]*preview"[^>]*data-testid="export-code"/);
  });

  it('shows the stylesheet, under its own size, when the person chose that file', () => {
    const out = body({ view: { ...START, shown: 'css' } });
    expect(out).toContain(':root{}');
    expect(out).toContain('7 bytes');
  });

  it('has the file switch for the Tailwind form only', () => {
    expect(body({})).toContain('data-testid="export-file"');
    expect(body({ dialog: dialog({ format: 'css', css: '' }) })).not.toContain('data-testid="export-file"');
  });

  it('says the sizes of text are measured as the canvas does and that the opening browser lays the page out, and names the stylesheet by what the app layer gave', () => {
    const out = body({ dialog: dialog({ cssFile: 'site.css' }) });
    expect(out).toMatch(/Text sizes are measured in this browser, with the font the canvas draws/);
    expect(out).toMatch(/laid out by the browser that opens it/);
    expect(out).toMatch(/tab, a link or CJK characters can break differently there \(Warnings names the ones it detects\)/);
    expect(out).not.toMatch(/Text sizes are (estimated|not measured)/);
    expect(out).toContain('site.css');
    expect(out).not.toContain('meldframe.css');
  });

  it('uses the words of the glossary: the HTML, not "the document"', () => {
    expect(body({})).not.toMatch(/\bdocument\b/i);
  });

  it('a frame over the limit gives an alert with both sizes, no code, and neither Copy nor Download', () => {
    const out = body({ dialog: dialog({ error: 'E_EXPORT_TOO_LARGE', html: '', css: '', htmlBytes: 0, cssBytes: 0, countedBytes: 400_000 }) });
    expect(out).toMatch(/role="alert"[^>]*data-testid="export-error"|data-testid="export-error"[^>]*role="alert"/);
    expect(out).toContain('390.6 KiB');
    expect(out).not.toContain('data-testid="export-code"');
    expect(disabled(tag(out, 'export-copy'))).toBe(true);
    expect(disabled(tag(out, 'export-download'))).toBe(true);
  });

  it('puts the review note above the buttons when batches wait, with a Review button, and none when they do not', () => {
    const out = body({ unreviewed: 2 });
    expect(out).toMatch(/data-testid="export-review"[^>]*>[\s\S]*2 batches are not reviewed yet/);
    expect(out).toMatch(/data-testid="export-review"[\s\S]*<button[^>]*>Review/);
    expect(body({ unreviewed: 0 })).not.toContain('data-testid="export-review"');
  });

  it('shows the review note with Review when the count is unknown, and says unknown, not nothing waiting', () => {
    const out = body({ unreviewed: null });
    expect(out).toMatch(/data-testid="export-review"[^>]*>[\s\S]*not known whether agent work is waiting/);
    expect(out).toMatch(/data-testid="export-review"[\s\S]*<button[^>]*>Review/);
  });

  it('draws no size for a dialog with no file, and not 0 bytes', () => {
    const out = body({ dialog: dialog({ error: 'E_EXPORT_TOO_LARGE', html: '', css: '', htmlBytes: null, cssBytes: null, countedBytes: 400_000 }) });
    expect(out).toContain('data-testid="export-error"');
    expect(out).not.toContain('0 bytes');
  });

  it('lists what the export could not carry with the node, and says the file is unchanged', () => {
    const out = body({ dialog: dialog({ unsupported: [{ code: 'on_text', node: 't1', nodeName: 'Hero Title', message: 'a text node paints no shadows' }] }) });
    expect(out).toContain('data-testid="export-unsupported"');
    expect(out).toContain('Hero Title');
    expect(out).toContain('on_text');
    expect(out).toContain('Your file is unchanged.');
    expect(body({})).not.toContain('data-testid="export-unsupported"');
  });

  it('draws a node name with markup and with characters that reorder text as plain text', () => {
    const out = body({ dialog: dialog({ unsupported: [{ code: 'c', node: 'n', nodeName: '<img src=x>gnp‮.exe', message: 'm⁦x' }] }) });
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;img src=x&gt;gnp.exe');
    expect(out).not.toMatch(/[‪-‮⁦-⁩]/);
  });
});

describe('the tabs and the PNG tab', () => {
  it('has the HTML and the PNG tab, the chosen one selected, and no PDF', () => {
    const out = body({});
    expect(out).toMatch(/id="export-tab-html"[^>]*aria-selected="true"|aria-selected="true"[^>]*id="export-tab-html"/);
    expect(out).toMatch(/id="export-tab-png"[^>]*aria-selected="false"|aria-selected="false"[^>]*id="export-tab-png"/);
    expect(out).not.toMatch(/PDF/);
    expect(body({ view: { ...START, tab: 'png' } })).toMatch(/id="export-tab-png"[^>]*aria-selected="true"|aria-selected="true"[^>]*id="export-tab-png"/);
  });

  it('shows the file and size of the PNG of the frame it is given, and exports only when there is a plan', () => {
    const png = { ...START, tab: 'png' as const };
    const out = body({ view: png });
    expect(out).toContain('Landing Desktop.png · 1440 × 900 px');
    expect(disabled(tag(out, 'export-png'))).toBe(false);
    const big = body({ view: png, scale: 4, plan: { ok: false, why: 'too_large', asked: { scale: 4, width: 5760, height: 3600 }, fits: { scale: 3, width: 4320, height: 2700 } } });
    expect(disabled(tag(big, 'export-png'))).toBe(true);
    expect(big).toContain('At 4x the image would be 5760 × 3600 px');
    expect(big).toContain('The largest scale that fits is 3x (4320 × 2700 px).');
    expect(body({ view: png, plan: null })).toContain('There is nothing to export.');
  });

  it('offers 0.5x to 4x on the PNG tab', () => {
    expect(scales(body({ view: { ...START, tab: 'png' } }))).toEqual(['0.5x', '1x', '2x', '3x', '4x']);
  });

  it('does not say text sizes are estimated on the PNG tab: the canvas draws that with the real text engine', () => {
    expect(body({ view: { ...START, tab: 'png' } })).not.toMatch(/estimated/);
  });

  it('wires each tab to its panel, as the Svelte copy does', () => {
    expect(body({})).toMatch(/id="export-panel-html"[^>]*role="tabpanel"[^>]*aria-labelledby="export-tab-html"|role="tabpanel"[^>]*aria-labelledby="export-tab-html"[^>]*id="export-panel-html"|aria-labelledby="export-tab-html"/);
    expect(body({ view: { ...START, tab: 'png' } })).toContain('aria-labelledby="export-tab-png"');
  });
});

describe('the dialog', () => {
  it('draws nothing while closed: the popup is made when it opens, in a portal', () => {
    const out = draw(<ExportDialog dialog={null} unreviewed={0} scale={1} onScale={() => {}} plan={null} onPng={() => {}} dispatch={() => {}} onReview={() => {}} />);
    expect(out).toBe('');
  });
});

describe('the Export section', () => {
  const target = { id: 'f1', name: 'Landing Desktop' };
  const section = (props: Partial<ExportSectionProps>) => draw(<ExportSection target={target} scale={1} onScale={() => {}} plan={ok} onPng={() => {}} onHtml={() => {}} {...props} />);

  it('is not there when no frame can be exported', () => {
    const out = section({ target: null, plan: null });
    expect(out).not.toContain('data-testid="export-section"');
    expect(out).not.toContain('Export as PNG');
  });

  it('offers the PNG with its file and size, and the HTML dialog, for the selected frame', () => {
    const out = section({ plan: { ok: true, file: 'Landing Desktop.png', width: 1440, height: 2200 } });
    expect(out).toContain('data-testid="export-section"');
    expect(out).toContain('Landing Desktop.png · 1440 × 2200 px');
    expect(tag(out, 'export-section-png')).not.toBe('');
    expect(disabled(tag(out, 'export-section-png'))).toBe(false);
    expect(out).toContain('HTML + Tailwind…');
    expect(out).toMatch(/aria-label="PNG scale"/);
  });

  it('offers 0.5x, 1x, 2x, 3x and 4x, and marks the chosen scale', () => {
    expect(scales(section({}))).toEqual(['0.5x', '1x', '2x', '3x', '4x']);
    for (const s of [0.5, 1, 2, 3, 4] as const) {
      const checked = radios(section({ scale: s })).filter((b) => b.includes('aria-checked="true"')).map((b) => /([\d.]+)x/.exec(b)?.[1]);
      expect(checked).toEqual([String(s)]);
    }
  });

  it('cannot export a PNG that would be too large, and says why and which scale fits', () => {
    const plan = { ok: false as const, why: 'too_large' as const, asked: { scale: 4 as const, width: 5760, height: 3600 }, fits: { scale: 3 as const, width: 4320, height: 2700 } };
    const out = section({ plan, scale: 4 });
    expect(disabled(tag(out, 'export-section-png'))).toBe(true);
    expect(out).toContain('At 4x the image would be 5760 × 3600 px, more than the limit of 4096 × 4096 px in area.');
    expect(out).toContain('The largest scale that fits is 3x (4320 × 2700 px).');
  });

  it('draws the name of the frame nowhere as markup (the plan carries it as text)', () => {
    expect(section({ plan: { ok: true, file: '<b>x</b>.png', width: 1, height: 1 } })).not.toContain('<b>x</b>');
  });
});
