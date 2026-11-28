// The Import variables and Export variables dialogs: the words they say and the small decisions of the page. Pure.
// What is imported or exported, what is refused and why are the app layer's (`state.variablesIo`: the text of the
// export, the result and the report of the import); this file only words them. The rules (the limits, the duplicates,
// the format) are in Rust and arrive as codes and numbers.
import type { ImportEntry, VariablesExportDialog, VariablesImportResult } from '@meldframe/protocol';
import { nameOf } from '@meldframe/web-document/names';
import { noticeFor } from '@meldframe/web-document/notices';
import { findingRows, previewOf, sizeText } from '@meldframe/web-export/view';
import { MAX_COLLECTIONS, MAX_MODES, MAX_VARIABLES } from '@meldframe/web-variables/manage';

// What the export dialog draws of the web-export view (the preview with its line cap, the rows of the warnings, the words for a size)
// comes through here, so that both copies of the body read it from one place.
export { findingRows, previewOf, sizeText };

/** The name the exported text is saved under. */
export const EXPORT_FILE = 'variables.css';

/**
 * The most bytes of a file this page reads into the field. This protects the browser, it is not the import limit: that is
 * the app layer's (`limit` of the result) and a text over it is refused there, with both sizes.
 */
export const READ_CAP = 1024 * 1024;

/**
 * The most variables the dialog imports at a time: `MAX_VARIABLES` of `meldframe_app::variables_import_dialog`, which refuses more as
 * `E_IMPORT_TOO_MANY` (a test reads the number from its source). The result carries it too (`maxVariables`).
 */
export const IMPORT_CAP = 50;

export const plural = (n: number, one: string, many = `${one}s`): string => `${n.toLocaleString('en-US')} ${n === 1 ? one : many}`;

/** What the export has in it, in one line; null when there is no text. */
export function exportSummary(d: VariablesExportDialog): string | null {
  if (d.error) return null;
  if (d.variables === 0) return null;
  return `${plural(d.variables, 'variable')} in ${plural(d.collections, 'collection')}, ${plural(d.blocks, 'rule')}, ${sizeText(d.bytes)}`;
}

/** Why there is no text, in words; null when there is. */
export function exportProblem(d: VariablesExportDialog): string | null {
  if (!d.error) return null;
  if (d.error === 'E_EXPORT_TOO_LARGE') return `These variables are too many to export as one text: it would be ${d.bytes === null ? 'of a size not known' : sizeText(d.bytes)} and the limit is ${sizeText(d.limit)}. Remove variables you do not need.`;
  return `The variables could not be exported (${d.error}).`;
}

/** What the person is told when the file chosen cannot be read; null when it can. */
export function fileProblem(size: number): string | null {
  return size > READ_CAP ? `This file is ${sizeText(size)}. A style sheet of more than ${sizeText(READ_CAP)} is not read here: send it in parts.` : null;
}

/** The result of the last import, worded. `tone` picks the colours; `lines` are the sentences under the title. */
export interface ImportWords {
  tone: 'ok' | 'neutral' | 'error';
  title: string;
  lines: string[];
}

const ENTITY: Record<string, string> = { collection: 'collection', mode: 'mode', variable: 'variable' };

export function importWords(r: VariablesImportResult): ImportWords {
  const rep = r.report;
  if (r.status === 'imported') {
    // A report is there whenever the text was read; if it is not, the counts are not known and are not drawn as zero.
    const lines = [
      rep === null
        ? 'The variables were added. The counts of what was added are not known.'
        : `${plural(rep.collections, 'collection')}, ${plural(rep.modes, 'mode')} and ${plural(rep.variables, 'variable')} were added: ${plural(rep.colors, 'colour')} and ${plural(rep.numbers, 'number')}, ${plural(rep.aliases, 'alias', 'aliases')} among them.`,
    ];
    if (rep !== null && rep.skipped > 0) lines.push(`${plural(rep.skipped, 'custom property', 'custom properties')} ${rep.skipped === 1 ? 'was' : 'were'} left out, and the report below says why.`);
    lines.push('One undo takes the whole import back.');
    return { tone: 'ok', title: 'Imported', lines };
  }
  if (r.status === 'nothing') {
    return { tone: 'neutral', title: 'Nothing to import', lines: ['No variable was found in this text that the file can hold, so nothing was added.', ...(rep && rep.entries.length ? ['The report below says what was skipped.'] : [])] };
  }
  return { tone: 'error', title: 'Nothing was imported', lines: [refusal(r)] };
}

function refusal(r: VariablesImportResult): string {
  const code = r.code ?? '';
  if (code === 'empty_text') return 'There is no text to import. Paste CSS into the field or choose a file.';
  if (code === 'E_IMPORT_TOO_LARGE') return `This text is ${sizeText(r.bytes)} and an import reads at most ${sizeText(r.limit)}. Send the style sheet in parts.`;
  if (code === 'E_DUPLICATE_ID' || code === 'E_DUPLICATE_NAME') {
    const what = r.at ? `The ${ENTITY[r.at.entity] ?? 'item'} ${nameOf(r.at.name)}` : 'Something in this text';
    return `${what} is already in this file. An import only adds: it does not change what the file holds, and a second import of the same names is refused. Edit the existing variables in Manage variables.`;
  }
  // The count of the report stops at the 5,000 of a file, so the text is only said to be over the cap.
  if (code === 'E_IMPORT_TOO_MANY') return `This text has more than ${plural(r.maxVariables, 'variable')} and the dialog imports at most ${r.maxVariables} at a time, so that every change reaches the server. Split the text, or ask your agent to import it with the tool import_tokens, which reads the same CSS in parts.`;
  if (code === 'E_TOO_MANY_ENTRIES') return `Adding this would go over a limit of the file: at most ${plural(MAX_COLLECTIONS, 'collection')}, ${plural(MAX_MODES, 'mode')} in a collection and ${plural(MAX_VARIABLES, 'variable')}. Remove some first, or import less.`;
  return noticeFor({ type: 'refused', code }) ?? `The import was refused (${code}).`;
}

export interface EntryRow {
  key: string;
  kind: ImportEntry['kind'];
  name: string;
  code: string;
  message: string;
}

/** The lines of the report. The app layer made every text of it safe to show (the importers clean it once, ADR-0020); nothing here cleans it again. */
export function entryRows(list: readonly ImportEntry[]): EntryRow[] {
  return list.map((e, i) => ({ key: `${e.code}:${e.name}:${i}`, kind: e.kind, name: e.name, code: e.code, message: e.message }));
}

/** The count line under the report when it was cut at its cap. */
export const omittedText = (n: number): string | null => (n > 0 ? `${plural(n, 'more line')} not listed.` : null);

/** The help under the import field: what is read, what is not run, what the names become. */
export const IMPORT_HELP = [
  'Reads the custom properties of :root, of a dark rule (.dark, [data-theme=dark] or a prefers-color-scheme: dark query) and of a Tailwind v4 @theme block. Nothing in the text is run or fetched, and the file does not leave your browser.',
  `At most ${IMPORT_CAP} variables at a time: for a longer style sheet, ask your agent to use the tool import_tokens, which reads the same CSS in parts.`,
  'The import only adds, and one undo takes it all back. The names are read as written (--brand-500 is the variable brand-500, in the collection root or theme).',
];

/** The help under the export text: the format, and the known limit of a round trip. */
export const EXPORT_HELP = [
  'CSS custom properties, one per line: the default mode of each collection in :root and every other mode in its own rule. DTCG and Tailwind @theme output come later.',
  'Names are written as --collection-name. Import variables reads --name, so this text imported again gives names with the collection in front.',
  'A colour is written as its stored hex, lowercased, alpha included.',
];
