import { describe, expect, it } from 'vitest';
import type { VariablesExportDialog } from '@meldframe/protocol';
import { EXPORT_FILE, EXPORT_HELP, exportProblem, exportSummary, plural } from './io';

const exp = (over: Partial<VariablesExportDialog> = {}): VariablesExportDialog => ({ css: ':root {\n}\n', collections: 2, variables: 5, blocks: 3, bytes: 2048, limit: 262144, warnings: [], ...over });

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

describe('the plural', () => {
  it(' keeps one and many apart', () => {
    expect([plural(1, 'mode'), plural(2, 'mode'), plural(1000, 'variable'), plural(2, 'alias', 'aliases')]).toEqual(['1 mode', '2 modes', '1,000 variables', '2 aliases']);
  });
});
