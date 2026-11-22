// The Export variables dialog: the words it says. Pure. The text, its counts and its limit are the app layer's
// (`state.variablesIo.export`); this file only words them. The rules (the format, the limit) are in Rust and arrive as
// numbers and codes.
import type { VariablesExportDialog } from '@meldframe/protocol';
import { sizeText } from '@meldframe/web-export/view';
import { plain } from '@meldframe/web-variables/manage';

export { plain, sizeText };

/** The name the exported text is saved under. */
export const EXPORT_FILE = 'variables.css';

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
  if (d.error === 'E_EXPORT_TOO_LARGE') return `These variables are too many to export as one text: it would be ${sizeText(d.bytes)} and the limit is ${sizeText(d.limit)}. Remove variables you do not need.`;
  return `The variables could not be exported (${d.error}).`;
}

/** The help under the export text: the format, and the known limit of a round trip. */
export const EXPORT_HELP = [
  'CSS custom properties, one per line: the default mode of each collection in :root and every other mode in its own rule. DTCG and Tailwind @theme output come later.',
  'Names are written as --collection-name. Import variables reads --name, so this text imported again gives names with the collection in front.',
  'A colour is written as its stored hex, lowercased, alpha included.',
];
