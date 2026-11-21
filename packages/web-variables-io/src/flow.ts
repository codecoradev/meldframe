// The view state of the Export variables and Import variables dialogs, as pure steps. What each dialog shows is the app
// layer's (`state.variablesIo`); this is what the page keeps on top of it, and the rules for when that is put away.

/** What the last Copy said, and the text it was about. */
export interface Said {
  ok: boolean;
  text: string;
  /** The text that was copied (or that the browser refused to copy). */
  css: string;
}

/** What the last Copy said, while it is still about the text on view: a text that changed under an open dialog (another actor added a variable) makes it stale. */
export function saidFor(said: Said | null, css: string): { ok: boolean; text: string } | null {
  return said !== null && said.css === css ? { ok: said.ok, text: said.text } : null;
}
