// What the browser does with an export: save a file, copy text. The only DOM here; the decisions are in view.ts and png.ts.

/** Save a blob under a file name, as a click on a link with `download` does. */
export function saveBlob(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = fileName;
  document.body.append(a);
  a.click();
  a.remove();
  // Some browsers read the URL after the click returns: keep it a while.
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

export function saveText(text: string, fileName: string, mime: string): void {
  saveBlob(new Blob([text], { type: `${mime};charset=utf-8` }), fileName);
}

/** Put text on the clipboard; false when the browser refuses (no permission, an insecure page). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
