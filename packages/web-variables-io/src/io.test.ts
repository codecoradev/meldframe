import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { ImportReport, VariablesExportDialog, VariablesImportResult } from '@meldframe/protocol';
import { EXPORT_FILE, EXPORT_HELP, IMPORT_CAP, IMPORT_HELP, READ_CAP, entryRows, exportProblem, exportSummary, fileProblem, importWords, omittedText, plural } from './io';

const exp = (over: Partial<VariablesExportDialog> = {}): VariablesExportDialog => ({ css: ':root {\n}\n', collections: 2, variables: 5, blocks: 3, bytes: 2048, limit: 262144, warnings: [], ...over });
const report = (over: Partial<ImportReport> = {}): ImportReport => ({ collections: 2, modes: 3, variables: 6, colors: 4, numbers: 2, aliases: 1, skipped: 0, entries: [], omitted: 0, ...over });
const result = (over: Partial<VariablesImportResult> = {}): VariablesImportResult => ({ status: 'imported', bytes: 100, limit: 262144, maxVariables: 50, report: report(), ...over });

describe('the Export variables words', () => {
  it('sum up what the text holds and its size', () => {
    expect(exportSummary(exp())).toBe('5 variables in 2 collections, 3 rules, 2.0 KiB');
    expect(exportSummary(exp({ variables: 1, collections: 1, blocks: 1 }))).toMatch(/^1 variable in 1 collection, 1 rule,/);
  });
  it('have no summary for a file with no variables, and none for an error', () => {
    expect(exportSummary(exp({ variables: 0 }))).toBeNull();
    expect(exportSummary(exp({ error: 'E_EXPORT_TOO_LARGE' }))).toBeNull();
  });
  it('say why a text is too large with both sizes, and name any other code', () => {
    expect(exportProblem(exp())).toBeNull();
    const big = exportProblem(exp({ error: 'E_EXPORT_TOO_LARGE', bytes: 300000 }));
    expect(big).toContain('293.0 KiB');
    expect(big).toContain('256.0 KiB');
    expect(exportProblem(exp({ error: 'E_X' }))).toContain('E_X');
  });
  it('save the text under variables.css and say the limits of the format', () => {
    expect(EXPORT_FILE).toBe('variables.css');
    expect(EXPORT_HELP.join(' ')).toMatch(/DTCG/);
    expect(EXPORT_HELP.join(' ')).toMatch(/--collection-name/);
    expect(EXPORT_HELP.join(' ')).toMatch(/collection in front/);
    expect(EXPORT_HELP.join(' ')).toMatch(/lowercased/);
  });
});

describe('the Import variables words', () => {
  it('count what was added and say that one undo takes it back', () => {
    const w = importWords(result());
    expect(w.tone).toBe('ok');
    expect(w.title).toBe('Imported');
    expect(w.lines[0]).toBe('2 collections, 3 modes and 6 variables were added: 4 colours and 2 numbers, 1 alias among them.');
    expect(w.lines.at(-1)).toMatch(/one undo/i);
  });
  it('mention what was left out, with the report as the place to read why', () => {
    const w = importWords(result({ report: report({ skipped: 3 }) }));
    expect(w.lines.join(' ')).toMatch(/3 custom properties were left out/);
    expect(importWords(result({ report: report({ skipped: 1 }) })).lines.join(' ')).toMatch(/1 custom property was left out/);
  });
  it('say that text with no variable adds nothing', () => {
    const w = importWords(result({ status: 'nothing', report: report({ collections: 0, variables: 0 }) }));
    expect(w.tone).toBe('neutral');
    expect(w.title).toBe('Nothing to import');
  });
  it('word each refusal and say that nothing was imported', () => {
    const refused = (over: Partial<VariablesImportResult>) => importWords(result({ status: 'refused', report: report({ collections: 0, variables: 0 }), ...over }));
    expect(refused({ code: 'empty_text' }).lines[0]).toMatch(/no text to import/i);
    const big = refused({ code: 'E_IMPORT_TOO_LARGE', bytes: 300 * 1024 });
    expect(big.lines[0]).toContain('300.0 KiB');
    expect(big.lines[0]).toContain('256.0 KiB');
    const dup = refused({ code: 'E_DUPLICATE_ID', at: { entity: 'collection', name: 'root' } });
    expect(dup.tone).toBe('error');
    expect(dup.title).toBe('Nothing was imported');
    expect(dup.lines[0]).toMatch(/^The collection root is already in this file/);
    expect(dup.lines[0]).toMatch(/only adds/);
    expect(refused({ code: 'E_DUPLICATE_ID', at: { entity: 'variable', name: 'brand' } }).lines[0]).toMatch(/^The variable brand/);
    const many = refused({ code: 'E_IMPORT_TOO_MANY', report: report({ variables: 120 }) });
    expect(many.lines[0]).toMatch(/^This text has more than 50 variables and the dialog imports at most 50 at a time/);
    // The report's count stops at 5,000 for a longer text: the message never quotes it.
    expect(refused({ code: 'E_IMPORT_TOO_MANY', report: report({ variables: 5000 }) }).lines[0]).not.toContain('5,000');
    expect(many.lines[0]).not.toContain('120');
    expect(many.lines[0]).toContain('import_tokens');
    expect(refused({ code: 'E_TOO_MANY_ENTRIES' }).lines[0]).toMatch(/5,000 variables/);
    expect(refused({ code: 'outbox_full' }).lines[0]).toMatch(/Too many changes are waiting/);
    expect(refused({ code: 'E_WHATEVER' }).lines[0]).toContain('E_WHATEVER');
  });
  it('draw the name of a duplicate as plain text', () => {
    const w = importWords(result({ status: 'refused', code: 'E_DUPLICATE_ID', at: { entity: 'variable', name: 'br‮and' } }));
    expect(w.lines[0]).toContain('brand');
    expect(w.lines[0]).not.toMatch(/‮/);
  });
  it('use the words of the glossary, never token or theme value', () => {
    const all = [result(), result({ status: 'nothing' }), result({ status: 'refused', code: 'E_DUPLICATE_ID', at: { entity: 'mode', name: 'x' } })].flatMap((r) => [importWords(r).title, ...importWords(r).lines]).concat(IMPORT_HELP, EXPORT_HELP).join(' ');
    expect(all).not.toMatch(/\btokens?\b/i);
  });
});

describe('the lines of the report', () => {
  it('are plain text with their kind, name and message', () => {
    const rows = entryRows([{ kind: 'converted', code: 'rem', name: '--sp‮ace', message: '1rem is 16px' }, { kind: 'skipped', code: 'computed_value', name: '--w', message: 'calc()' }]);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ kind: 'converted', name: '--space', code: 'rem', message: '1rem is 16px' });
    expect(new Set(rows.map((r) => r.key)).size).toBe(2);
  });
  it('say how many lines were not listed', () => {
    expect(omittedText(0)).toBeNull();
    expect(omittedText(1)).toBe('1 more line not listed.');
    expect(omittedText(12)).toBe('12 more lines not listed.');
  });
});

describe('a file chosen', () => {
  it('is read up to the cap that protects the browser and refused past it, with its size', () => {
    expect(fileProblem(READ_CAP)).toBeNull();
    expect(fileProblem(READ_CAP + 1)).toMatch(/1\.0 MiB/);
  });
  it('plural keeps one and many apart', () => {
    expect([plural(1, 'mode'), plural(2, 'mode'), plural(1000, 'variable'), plural(2, 'alias', 'aliases')]).toEqual(['1 mode', '2 modes', '1,000 variables', '2 aliases']);
  });
});

describe('the cap of the dialog', () => {
  it('is the number in the Rust source, so a change there fails here', () => {
    const rs = readFileSync(new URL('../../../crates/features/variables-import/variables-import-dialog/src/lib.rs', import.meta.url), 'utf8');
    expect(Number(/pub const MAX_VARIABLES: usize = (\d+);/.exec(rs)![1])).toBe(IMPORT_CAP);
  });
  it('is in the help, with the tool that takes a longer style sheet', () => {
    expect(IMPORT_HELP.join(' ')).toContain(`At most ${IMPORT_CAP} variables at a time`);
    expect(IMPORT_HELP.join(' ')).toContain('import_tokens');
  });
});
