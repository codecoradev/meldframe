// The host calls of the Export variables dialog: put the text on the clipboard and save it as a file. The only DOM of the export
// besides the dialog itself; both copies of the dialog call these, and what Copy says is worded here once. Nothing leaves the browser.
import { copyText, saveText } from '@meldframe/web-export/save';
import type { Said } from './flow';
import { EXPORT_FILE } from './io';

/** Copies `css` and says how it went, with the text it was about (a text that changes under the open dialog makes the answer stale: `saidFor`). */
export async function copyExport(css: string): Promise<Said> {
  return (await copyText(css)) ? { ok: true, text: 'Copied.', css } : { ok: false, text: 'Your browser did not allow copying.', css };
}

/** Saves `css` as `variables.css`. */
export const saveExport = (css: string): void => saveText(css, EXPORT_FILE, 'text/css');
