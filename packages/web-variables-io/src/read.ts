// Reading the file a person chooses into the Import field. The only DOM of the import besides the dialog itself: local,
// nothing is uploaded or sent anywhere.
import { fileProblem } from './io';

export type Read = { ok: true; text: string } | { ok: false; error: string };

/** The text of a file, or why it was not read: too large for the browser's care, or the browser could not read it. */
export function readFile(file: File): Promise<Read> {
  const problem = fileProblem(file.size);
  if (problem) return Promise.resolve({ ok: false, error: problem });
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === 'string' ? { ok: true, text: reader.result } : { ok: false, error: 'This file could not be read as text.' });
    reader.onerror = () => resolve({ ok: false, error: 'This file could not be read.' });
    reader.readAsText(file);
  });
}
