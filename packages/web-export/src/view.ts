// The words and the display rules of the Export dialog. Pure. The document, the report, the limit and the names of the
// files are the app layer's (`state.export.dialog`); this only words them. Nothing here cleans a text.
import type { ExportDialog, ExportFinding } from '@meldframe/protocol';

/** How many lines of the document the preview draws; copy and download take all of it. */
export const PREVIEW_LINES = 400;

/** The first `max` lines of a document (no empty line for a final line break), and how many it has. */
export function previewOf(text: string, max = PREVIEW_LINES): { lines: string[]; total: number } {
  const all = text === '' ? [] : text.replace(/\n$/, '').split('\n');
  return { lines: all.slice(0, max), total: all.length };
}

/**
 * The file the dialog shows, copies and saves: its name, its text, its media type and its size. The stylesheet of the
 * Tailwind form is saved under the name the document links, which the app layer gives (`cssFile`).
 */
export function fileOf(d: ExportDialog, shown: 'html' | 'css'): { name: string; text: string; mime: string; bytes: number | null } {
  return shown === 'css'
    ? { name: d.cssFile, text: d.css, mime: 'text/css', bytes: d.cssBytes }
    : { name: d.htmlFile, text: d.html, mime: 'text/html', bytes: d.htmlBytes };
}

/** A size in words; `null` (the app layer does not know it: there is no file) is said, never drawn as 0 bytes. */
export function sizeText(bytes: number | null): string {
  if (bytes === null) return 'Unknown size';
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${bytes} bytes`;
}

/** Why there is no document, in words; null when there is one. */
export function errorText(d: ExportDialog): string | null {
  if (!d.error) return null;
  if (d.error === 'E_EXPORT_TOO_LARGE') return `This frame is too large to export as HTML: the output would be ${d.countedBytes === null ? 'of a size not known' : sizeText(d.countedBytes)} and the limit is ${sizeText(d.limit)}. Select a smaller frame.`;
  return `This frame could not be exported (${d.error}).`;
}

/**
 * The note above the buttons when agent work is waiting for a review; null when nothing is. A count the app layer does not
 * know (the list of batches has not loaded, or failed to) is said as unknown: it is never read as nothing waiting.
 */
export function reviewText(unreviewed: number | null): string | null {
  if (unreviewed === null) return 'It is not known whether agent work is waiting for a review: the list of batches has not loaded. Export anyway or review first.';
  if (unreviewed <= 0) return null;
  return `${unreviewed} ${unreviewed === 1 ? 'batch is' : 'batches are'} not reviewed yet. Export anyway or review first.`;
}

export interface FindingRow {
  key: string;
  node: string;
  code: string;
  message: string;
}

/** The rows of the report: the node (its name, else its id, else "This file"), the stable code and the explanation. The app layer made every text safe to show. */
export function findingRows(list: readonly ExportFinding[]): FindingRow[] {
  return list.map((f, i) => ({ key: `${f.code}:${f.node ?? ''}:${i}`, node: f.nodeName || f.node || 'This file', code: f.code, message: f.message }));
}
