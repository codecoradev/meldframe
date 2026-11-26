import { readFileSync } from 'node:fs';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { ImportReport, VariablesExportDialog, VariablesImportResult } from '@meldframe/protocol';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ExportVariablesBody from './ExportVariablesBody.svelte';
import ImportVariablesBody from './ImportVariablesBody.svelte';

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

const report = (over: Partial<ImportReport> = {}): ImportReport => ({ collections: 1, modes: 1, variables: 2, colors: 1, numbers: 1, aliases: 0, skipped: 0, entries: [], omitted: 0, ...over });
const res = (over: Partial<VariablesImportResult> = {}): VariablesImportResult => ({ status: 'imported', bytes: 10, limit: 262144, maxVariables: 50, report: report(), ...over });
const importBody = (props: Record<string, unknown> = {}) =>
  render(TestHost as never, { props: { of: ImportVariablesBody, props: { result: null, text: '', onText() {}, note: null, onFile() {}, onClear() {}, onImport() {}, onClose() {}, ...props } } as never }).body;

describe('the Import variables dialog body', () => {
  it('has a labelled field, a file chooser, the help, and Import off while the field is empty', () => {
    const out = importBody();
    expect(out).toMatch(/<label for="import-vars-text"[^>]*>CSS to import<\/label>/);
    expect(out).toMatch(/<textarea[^>]*id="import-vars-text"/);
    expect(out).not.toMatch(/<textarea[^>]*aria-label/);
    expect(out).toContain('data-testid="import-vars-file"');
    expect(out).toContain('type="file"');
    expect(out).toContain('Choose a file');
    expect(out).toContain('only adds');
    expect(out).toContain('does not leave your browser');
    expect(disabled(tag(out, 'import-vars-import'))).toBe(true);
    expect(disabled(tag(importBody({ text: ':root{}' }), 'import-vars-import'))).toBe(false);
    expect(disabled(tag(importBody({ text: '  \n ' }), 'import-vars-import'))).toBe(true);
  });
  it('draws the text of the field as the field, not as markup', () => {
    const out = importBody({ text: '</textarea><script>alert(1)</script>' });
    expect(out).not.toContain('<script>alert');
  });
  it('shows the counts and the report of an import that was applied', () => {
    const out = importBody({ result: res({ report: report({ skipped: 1, entries: [{ kind: 'skipped', code: 'computed_value', name: '--w', message: 'calc() is not evaluated' }] }) }) });
    expect(out).toContain('data-status="imported"');
    expect(out).toContain('1 collection, 1 mode and 2 variables were added');
    expect(out).toContain('data-testid="import-vars-report"');
    expect(out).toContain('calc() is not evaluated');
    expect(out).toContain('data-kind="skipped"');
  });
  it('shows a refusal in the one live region of the result, not in an alert inside a status, which would be read twice', () => {
    const out = importBody({ result: res({ status: 'refused', code: 'E_DUPLICATE_ID', at: { entity: 'collection', name: 'root' } }) });
    expect(out).not.toContain('role="alert"');
    expect(out.match(/data-testid="import-vars-result"/g)).toHaveLength(1);
    expect(out).toMatch(/role="status" aria-live="polite" data-testid="import-vars-result" data-status="refused"/);
    expect(out).toContain('The collection root is already in this file');
    expect(out).not.toContain('data-testid="import-vars-report"');
  });
  it('says so for text with no variable, with the report', () => {
    const out = importBody({ result: res({ status: 'nothing', report: report({ variables: 0, entries: [{ kind: 'note', code: 'x', name: '.card', message: 'a rule that is not the root' }] }) }) });
    expect(out).toContain('Nothing to import');
    expect(out).toContain('a rule that is not the root');
  });
  it('draws every text of the report as text, as the app layer gave it, and says how many lines were left out', () => {
    const out = importBody({ result: res({ report: report({ omitted: 7, entries: [{ kind: 'skipped', code: 'x', name: '<img src=x>', message: 'a b' }] }) }) });
    expect(out).toContain('&lt;img src=x>');
    expect(out).not.toContain('<img src=x');
    expect(out).toContain('7 more lines not listed');
  });
  it('draws no report and no counts for text that was not read (the result has none), instead of an empty one', () => {
    const out = importBody({ result: res({ status: 'refused', code: 'empty_text', report: null }) });
    expect(out).toContain('no text to import');
    expect(out).not.toContain('not listed');
  });
  it('keeps the note about the file in a status region', () => {
    expect(importBody({ note: { ok: false, text: 'This file is 2.0 MiB.' } })).toMatch(/role="status"[^>]*data-testid="import-vars-note">This file is 2\.0 MiB\./);
  });
});

describe('the sources', () => {
  it('never use markup from a file or a name: no raw html', () => {
    for (const f of ['ExportVariablesBody', 'ExportVariablesDialog', 'ImportVariablesBody', 'ImportVariablesDialog']) {
      expect(readFileSync(new URL(`./${f}.svelte`, import.meta.url), 'utf8'), f).not.toContain('{@html');
    }
  });
});

describe('Clear and the cap in the Import variables dialog body', () => {
  it('has a Clear button, off while the field is empty', () => {
    expect(disabled(tag(importBody(), 'import-vars-clear'))).toBe(true);
    expect(disabled(tag(importBody({ text: ':root{}' }), 'import-vars-clear'))).toBe(false);
  });
  it('says the cap in the help and points at the tool for a longer style sheet', () => {
    const out = importBody();
    expect(out).toContain('At most 50 variables at a time');
    expect(out).toContain('import_tokens');
  });
  it('words the refusal of a text over the cap with the cap (not a count) and the pointer to the tool', () => {
    const out = importBody({ result: res({ status: 'refused', code: 'E_IMPORT_TOO_MANY', maxVariables: 50, report: report({ variables: 51 }) }) });
    expect(out).toContain('This text has more than 50 variables');
    expect(out).toContain('at most 50 at a time');
    expect(out).toContain('import_tokens');
    expect(out).not.toContain('data-testid="import-vars-report"');
  });
});
