// The React bodies of the Export variables and Import variables dialogs, checked as io-view.test.ts checks the Svelte ones: the markup
// of a server render, in Node, and compared with the Svelte copy's render token by token (`tokens`: tags, attributes, classes, text), so
// that a class, a role, a name or a word that one copy has and the other does not fails here. What a server render cannot show (the
// dialogs around them, the focus, the keys, the file chooser, the clipboard) is the job of packages/web-variables-io/scripts/io-e2e.mjs
// and io-dialog-e2e.mjs with MELDFRAME_UI=react, and the real pixels that of packages/web-panels/scripts/pixels-e2e.mjs.
import type { ImportReport, VariablesExportDialog, VariablesImportResult } from '@meldframe/protocol';
import { tokens } from '@meldframe/web-chrome/react/markup.testutil';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import ExportVariablesBodySvelte from '../ExportVariablesBody.svelte';
import ImportVariablesBodySvelte from '../ImportVariablesBody.svelte';
import { ExportBody, type ExportBodyProps } from './ExportBody';
import { ImportBody, type ImportBodyProps } from './ImportBody';

const noop = () => {};
const dialog = (over: Partial<VariablesExportDialog> = {}): VariablesExportDialog => ({ css: ':root {\n  --a-b: 12;\n}\n', collections: 1, variables: 1, blocks: 1, bytes: 26, limit: 262144, warnings: [], ...over });
const exportProps = (props: Record<string, unknown> = {}) => ({ dialog: dialog(), said: null, onCopy: noop, onDownload: noop, onClose: noop, ...props });
const exportReact = (props: Record<string, unknown> = {}) => renderToStaticMarkup(<ExportBody {...(exportProps(props) as unknown as ExportBodyProps)} />);
const exportSvelte = (props: Record<string, unknown> = {}) => render(TestHost as never, { props: { of: ExportVariablesBodySvelte, props: exportProps(props) } as never }).body;
const tag = (out: string, id: string) => new RegExp(`<button[^>]*data-testid="${id}"[^>]*>`).exec(out)?.[0] ?? '';
const disabled = (t: string) => /\sdisabled(?![:-])/.test(t);

const many = Array.from({ length: 500 }, (_, i) => `  --a-v${i}: 12;`).join('\n');
const EXPORTS: [string, Record<string, unknown>][] = [
  ['a text with its summary and help', {}],
  ['a file with no variables', { dialog: dialog({ css: '', variables: 0, collections: 0, blocks: 0 }) }],
  ['a text over the limit', { dialog: dialog({ css: '', variables: 0, error: 'E_EXPORT_TOO_LARGE', bytes: 300000 }) }],
  ['a text over the limit of a size not known', { dialog: dialog({ css: '', variables: 0, error: 'E_EXPORT_TOO_LARGE', bytes: null }) }],
  ['another error', { dialog: dialog({ css: '', variables: 0, error: 'E_OTHER' }) }],
  ['warnings, drawn as plain text', { dialog: dialog({ warnings: [{ code: 'invalid_colour', message: 'variable <b>x</b> holds rebeccapurple' }, { code: 'second', message: 'two' }] }) }],
  ['a long preview, cut', { dialog: dialog({ css: many, variables: 500, blocks: 2 }) }],
  ['a Copy that worked', { said: { ok: true, text: 'Copied.' } }],
  ['a Copy that the browser refused', { said: { ok: false, text: 'Your browser did not allow copying.' } }],
  ['several variables, collections and rules', { dialog: dialog({ variables: 1234, collections: 2, blocks: 3, bytes: 5000000 }) }],
];

describe('the React body of the Export variables dialog', () => {
  it('shows the CSS with line numbers, the summary and the help, with Copy and Download on', () => {
    const out = exportReact();
    expect(out).toContain('data-testid="export-vars-code"');
    expect(out).toContain('--a-b: 12;');
    expect(out).toContain('1 variable in 1 collection');
    expect(out).toContain('DTCG');
    expect(disabled(tag(out, 'export-vars-copy'))).toBe(false);
    expect(disabled(tag(out, 'export-vars-download'))).toBe(false);
    expect(out).toContain('aria-label="variables.css preview"');
    expect(out).toContain('tabindex="0"');
  });
  it('says what to do in a file with no variables, and has no Copy to press', () => {
    const out = exportReact({ dialog: dialog({ css: '', variables: 0, collections: 0, blocks: 0 }) });
    expect(out).toContain('data-testid="export-vars-empty"');
    expect(out).not.toContain('data-testid="export-vars-code"');
    expect(disabled(tag(out, 'export-vars-copy'))).toBe(true);
    expect(disabled(tag(out, 'export-vars-download'))).toBe(true);
  });
  it('a text over the limit is an alert with both sizes and neither Copy nor Download', () => {
    const out = exportReact({ dialog: dialog({ css: '', variables: 0, error: 'E_EXPORT_TOO_LARGE', bytes: 300000 }) });
    expect(out).toContain('role="alert"');
    expect(out).toContain('256.0 KiB');
    expect(out).not.toContain('E_EXPORT_TOO_LARGE');
    expect(disabled(tag(out, 'export-vars-copy'))).toBe(true);
  });
  it('lists what could not be written as stored, as plain text, and says the file is unchanged', () => {
    const out = exportReact({ dialog: dialog({ warnings: [{ code: 'invalid_colour', message: 'variable <b>x</b> holds rebeccapurple' }] }) });
    expect(out).toContain('data-testid="export-vars-warnings"');
    expect(out).toContain('&lt;b&gt;x&lt;/b&gt;');
    expect(out).not.toContain('<b>x');
    expect(out).toContain('Your file is unchanged');
  });
  it('cuts a long preview and says that Copy and Download take all of it, in one text', () => {
    const out = exportReact({ dialog: dialog({ css: many, variables: 500 }) });
    expect(out).toContain('first 400 of 500 lines');
    expect(out).not.toContain('<!-- -->');
  });
  it('says what the last Copy said, in a status region', () => {
    expect(exportReact({ said: { ok: true, text: 'Copied.' } })).toMatch(/role="status"[^>]*data-testid="export-vars-said">Copied\./);
  });
  it('is the same as the Svelte body in every state: tags, attributes, classes and words', () => {
    for (const [name, props] of EXPORTS) expect(tokens(exportReact(props)), name).toEqual(tokens(exportSvelte(props)));
  });
});

const report = (over: Partial<ImportReport> = {}): ImportReport => ({ collections: 1, modes: 1, variables: 2, colors: 1, numbers: 1, aliases: 0, skipped: 0, entries: [], omitted: 0, ...over });
const res = (over: Partial<VariablesImportResult> = {}): VariablesImportResult => ({ status: 'imported', bytes: 10, limit: 262144, maxVariables: 50, report: report(), ...over });
const importProps = (props: Record<string, unknown> = {}) => ({ result: null, text: '', onText: noop, note: null, onFile: noop, onClear: noop, onImport: noop, onClose: noop, ...props });
const importReact = (props: Record<string, unknown> = {}) => renderToStaticMarkup(<ImportBody {...(importProps(props) as unknown as ImportBodyProps)} />);
const importSvelte = (props: Record<string, unknown> = {}) => render(TestHost as never, { props: { of: ImportVariablesBodySvelte, props: importProps(props) } as never }).body;
const entries = [
  { kind: 'converted', code: 'converted_value', name: '--brand', message: 'a value was converted to the stored form' },
  { kind: 'skipped', code: 'computed_value', name: '--w', message: 'calc() is not evaluated' },
  { kind: 'note', code: 'x', name: '<img src=x>', message: 'a b' },
] as const;
const IMPORTS: [string, Record<string, unknown>][] = [
  ['an empty field', {}],
  ['text in the field', { text: ':root { --a: 12; }' }],
  ['text of spaces only', { text: '  \n ' }],
  ['text that would end a textarea', { text: '</textarea><script>alert(1)</script>' }],
  ['an import that was applied, with a report', { result: res({ report: report({ skipped: 1, entries: [...entries] }) }) }],
  ['an import that was applied and no counts', { result: res({ report: null }) }],
  ['one thing of each kind added', { result: res({ report: report({ collections: 1, modes: 1, variables: 1, colors: 1, numbers: 0, aliases: 1 }) }) }],
  ['a report cut at its cap', { result: res({ report: report({ omitted: 7, entries: [...entries] }) }) }],
  ['text with no variable', { result: res({ status: 'nothing', report: report({ variables: 0, entries: [{ kind: 'note', code: 'x', name: '.card', message: 'a rule that is not the root' }] }) }) }],
  ['text with no variable and no report', { result: res({ status: 'nothing', report: null }) }],
  ['a duplicate refused', { result: res({ status: 'refused', code: 'E_DUPLICATE_ID', at: { entity: 'collection', name: 'root' } }) }],
  ['a duplicate refused with no place', { result: res({ status: 'refused', code: 'E_DUPLICATE_NAME' }) }],
  ['a text over the cap', { result: res({ status: 'refused', code: 'E_IMPORT_TOO_MANY', maxVariables: 50, report: report({ variables: 51 }) }) }],
  ['a text too large', { result: res({ status: 'refused', code: 'E_IMPORT_TOO_LARGE', bytes: 300000, report: null }) }],
  ['an empty text refused', { result: res({ status: 'refused', code: 'empty_text', report: null }) }],
  ['a limit of the file', { result: res({ status: 'refused', code: 'E_TOO_MANY_ENTRIES', report: null }) }],
  ['a note about a file that was read', { text: 'x', note: { ok: true, text: 'Read theme.css (12 bytes). Check it, then import.' } }],
  ['a note about a file that was not read', { note: { ok: false, text: 'This file is 2.0 MiB.' } }],
];

describe('the React body of the Import variables dialog', () => {
  it('has a labelled field, a file chooser, the help, and Import off while the field is empty', () => {
    const out = importReact();
    expect(out).toMatch(/<label for="import-vars-text"[^>]*>CSS to import<\/label>/);
    expect(out).toMatch(/<textarea[^>]*id="import-vars-text"/);
    expect(out).not.toMatch(/<textarea[^>]*aria-label/);
    expect(out).toContain('data-testid="import-vars-file"');
    expect(out).toContain('type="file"');
    expect(out).toContain('Choose a file');
    expect(out).toContain('only adds');
    expect(out).toContain('does not leave your browser');
    expect(disabled(tag(out, 'import-vars-import'))).toBe(true);
    expect(disabled(tag(importReact({ text: ':root{}' }), 'import-vars-import'))).toBe(false);
    expect(disabled(tag(importReact({ text: '  \n ' }), 'import-vars-import'))).toBe(true);
  });
  it('draws the text of the field as the field, not as markup', () => {
    expect(importReact({ text: '</textarea><script>alert(1)</script>' })).not.toContain('<script>alert');
  });
  it('shows the counts and the report of an import that was applied, each count in one text', () => {
    const out = importReact({ result: res({ report: report({ skipped: 1, entries: [{ kind: 'skipped', code: 'computed_value', name: '--w', message: 'calc() is not evaluated' }] }) }) });
    expect(out).toContain('data-status="imported"');
    expect(out).toContain('1 collection, 1 mode and 2 variables were added');
    expect(out).toContain('data-testid="import-vars-report"');
    expect(out).toContain('data-kind="skipped"');
    expect(out).not.toContain('<!-- -->');
  });
  it('shows a refusal in the one live region of the result, not in an alert inside a status, which would be read twice', () => {
    const out = importReact({ result: res({ status: 'refused', code: 'E_DUPLICATE_ID', at: { entity: 'collection', name: 'root' } }) });
    expect(out).not.toContain('role="alert"');
    expect(out.match(/data-testid="import-vars-result"/g)).toHaveLength(1);
    expect(out).toMatch(/role="status"[^>]*aria-live="polite"|aria-live="polite"[^>]*role="status"/);
    expect(out).toContain('data-status="refused"');
    expect(out).toContain('The collection root is already in this file');
    expect(out).not.toContain('data-testid="import-vars-report"');
  });
  it('draws every text of the report as text, as the app layer gave it, and says how many lines were left out', () => {
    const out = importReact({ result: res({ report: report({ omitted: 7, entries: [{ kind: 'skipped', code: 'x', name: '<img src=x>', message: 'a b' }] }) }) });
    expect(out).toContain('&lt;img src=x&gt;');
    expect(out).not.toContain('<img src=x');
    expect(out).toContain('7 more lines not listed');
  });
  it('draws no report and no counts for text that was not read, instead of an empty one', () => {
    const out = importReact({ result: res({ status: 'refused', code: 'empty_text', report: null }) });
    expect(out).toContain('no text to import');
    expect(out).not.toContain('not listed');
  });
  it('words the refusal of a text over the cap with the cap (not a count) and the pointer to the tool', () => {
    const out = importReact({ result: res({ status: 'refused', code: 'E_IMPORT_TOO_MANY', maxVariables: 50, report: report({ variables: 51 }) }) });
    expect(out).toContain('This text has more than 50 variables');
    expect(out).toContain('import_tokens');
  });
  it('has a Clear button, off while the field is empty, and keeps the note about the file in a status region', () => {
    expect(disabled(tag(importReact(), 'import-vars-clear'))).toBe(true);
    expect(disabled(tag(importReact({ text: ':root{}' }), 'import-vars-clear'))).toBe(false);
    expect(importReact({ note: { ok: false, text: 'This file is 2.0 MiB.' } })).toMatch(/role="status"[^>]*data-testid="import-vars-note">This file is 2\.0 MiB\./);
  });
  it('is the same as the Svelte body in every state: tags, attributes, classes and words', () => {
    for (const [name, props] of IMPORTS) expect(tokens(importReact(props)), name).toEqual(tokens(importSvelte(props)));
  });
});
