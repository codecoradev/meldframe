// What the page says when the file did not open (`loadFile` failed, the network failed, the editor could not start).
// Pure. The detail is text from the server or the browser: it is made safe by the helper the page gives (`Safe`) and shown as
// plain text, never as markup; this file cleans nothing itself.
//
// Not reachable today, so not drawn: "File not found" (the server opens an unknown file as an empty one), "Session
// expired" (there is no sign-in yet), "Workspace is read-only" (no state for it). The storage limit is a toast
// (`quota_storage` in notices.ts), because it comes while the page is open.

export type FailureKind = 'no-access' | 'not-accepted' | 'unreachable' | 'server' | 'start';

export interface ErrorPageView {
  kind: FailureKind;
  title: string;
  text: string;
  /** What the server or the browser said, for the person who has to fix it. */
  detail?: string;
  /** The label of the one button: it loads the page again. */
  action?: string;
}

const AGAIN = 'Try again';

/**
 * Makes a text from the server or the browser safe to show and cuts it: the one helper of the app layer (`displayText` of
 * web-link, rule U9), given by the page, which has loaded the WASM module by then or has not. Null when it has not: no
 * text of the server or of the browser is shown, only what this page says itself.
 */
export type Safe = (text: string, maxChars: number) => string;

const BODY_MAX = 200;
const MESSAGE_MAX = 500;

/** What only a developer running `just dev` needs to read. */
const DEV_HINT = 'The dev token is missing or wrong: run `just dev` first, then `just web`.';

/** `LoadFailure` of web-document, read by its fields so this package does not import it. */
const statusOf = (e: unknown): { status: number; body: string } | null =>
  typeof e === 'object' && e !== null && typeof (e as { status?: unknown }).status === 'number' ? { status: (e as { status: number }).status, body: String((e as { body?: unknown }).body ?? '') } : null;

/**
 * The page for a failure of the start. Only a refusal or a missing answer of the snapshot request (`LoadFailure`) says
 * something about the server; any other error, a TypeError of a bug or of the WASM module that did not load among them,
 * is the editor failing to start and shows its own message. `dev` is true in a development build, which adds the hint for
 * a missing token to the 401 page.
 */
export function pageFor(e: unknown, opts: { dev: boolean; safe: Safe | null }): ErrorPageView {
  const r = statusOf(e);
  const body = r && opts.safe ? opts.safe(r.body, BODY_MAX) : '';
  const said = r ? `The server answered ${r.status}${body ? `: ${body}` : '.'}` : '';
  if (r?.status === 403) {
    return { kind: 'no-access', title: 'No access to this file', text: 'The server would not open this file for this page. Nothing was changed. Ask the owner to share it.', detail: said, action: AGAIN };
  }
  if (r?.status === 401) {
    return { kind: 'not-accepted', title: 'The server did not accept this page', text: 'The login this page sent was refused, so the file did not open. Nothing was changed.', detail: opts.dev ? `${said} ${DEV_HINT}` : said, action: AGAIN };
  }
  if (r?.status === 0) {
    return { kind: 'unreachable', title: "Can't reach the server", text: 'This page could not open the file, so nothing was changed. Check the connection and try again.', action: AGAIN };
  }
  if (r) {
    return { kind: 'server', title: 'The server could not open the file', text: 'Nothing was changed. Try again in a moment.', detail: said, action: AGAIN };
  }
  return { kind: 'start', title: 'The editor could not start', text: 'Nothing was changed.', detail: opts.safe ? opts.safe(e instanceof Error ? e.message : String(e), MESSAGE_MAX) || undefined : undefined, action: AGAIN };
}
