import { readFileSync } from 'node:fs';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { VariablesExportDialog } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ExportVariablesBody from './ExportVariablesBody.svelte';

const dialog = (over: Partial<VariablesExportDialog> = {}): VariablesExportDialog => ({ css: ':root {\n  --a-b: #fff;\n}\n', collections: 1, variables: 1, blocks: 1, bytes: 26, limit: 262144, warnings: [], ...over });
const exportBody = (props: Record<string, unknown> = {}) =>
  render(TestHost as never, { props: { of: ExportVariablesBody, props: { dialog: dialog(), said: null, onCopy() {}, onDownload() {}, onClose() {}, ...props } } as never }).body;
const tag = (out: string, id: string) => new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`).exec(out)?.[0] ?? '';
const disabled = (t: string) => /\sdisabled(?![:-])/.test(t);

describe('the Export variables dialog body', () => {
  it('shows the CSS with line numbers, the summary and the help, with Copy and Download on', () => {
    const out = exportBody();
    expect(out).toContain('data-testid="export-vars-code"');
    expect(out).toContain('--a-b: #fff;');
    expect(out).toContain('1 variable in 1 collection');
    expect(out).toContain('DTCG');
    expect(disabled(tag(out, 'export-vars-copy'))).toBe(false);
    expect(disabled(tag(out, 'export-vars-download'))).toBe(false);
    expect(out).toContain('aria-label="variables.css preview"');
  });
  it('says what to do in a file with no variables, and has no Copy to press', () => {
    const out = exportBody({ dialog: dialog({ css: '', variables: 0, collections: 0, blocks: 0 }) });
    expect(out).toContain('data-testid="export-vars-empty"');
    expect(out).not.toContain('data-testid="export-vars-code"');
    expect(disabled(tag(out, 'export-vars-copy'))).toBe(true);
    expect(disabled(tag(out, 'export-vars-download'))).toBe(true);
  });
  it('a text over the limit is an alert with both sizes and neither Copy nor Download', () => {
    const out = exportBody({ dialog: dialog({ css: '', variables: 0, error: 'E_EXPORT_TOO_LARGE', bytes: 300000 }) });
    expect(out).toContain('role="alert"');
    expect(out).toContain('256.0 KiB');
    expect(out).not.toContain('E_EXPORT_TOO_LARGE');
    expect(disabled(tag(out, 'export-vars-copy'))).toBe(true);
  });
  it('lists what could not be written as stored, as plain text, and says the file is unchanged', () => {
    const out = exportBody({ dialog: dialog({ warnings: [{ code: 'invalid_colour', message: 'variable <b>x</b> holds rebeccapurple' }] }) });
    expect(out).toContain('data-testid="export-vars-warnings"');
    expect(out).toContain('invalid_colour');
    expect(out).toContain('&lt;b>x&lt;/b>');
    expect(out).not.toContain('<b>x');
    expect(out).toContain('Your file is unchanged');
  });
  it('cuts a long preview and says that Copy and Download take all of it', () => {
    const css = Array.from({ length: 500 }, (_, i) => `  --a-v${i}: #fff;`).join('\n');
    const out = exportBody({ dialog: dialog({ css, variables: 500 }) });
    expect(out).toContain('data-testid="export-vars-cut"');
    expect(out).toContain('first 400 of 500 lines');
  });
  it('says what the last Copy said, in a status region', () => {
    expect(exportBody({ said: { ok: true, text: 'Copied.' } })).toMatch(/role="status"[^>]*data-testid="export-vars-said">Copied\./);
  });
});

describe('the sources', () => {
  it('never use markup from a file or a name: no raw html', () => {
    for (const f of ['ExportVariablesBody', 'ExportVariablesDialog']) {
      expect(readFileSync(new URL(`./${f}.svelte`, import.meta.url), 'utf8'), f).not.toContain('{@html');
    }
  });
});
