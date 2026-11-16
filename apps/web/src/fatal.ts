// What the page does when it cannot start or has to start over.

export const RELOAD_LIMIT = 3;
export const RELOAD_WINDOW_MS = 60_000;

interface Store {
  getItem(k: string): string | null;
  setItem(k: string, v: string): void;
}

/** True for at most RELOAD_LIMIT automatic reloads within a minute: a reload that diverges again must not loop. */
export function shouldReload(store: Store, now: number): boolean {
  try {
    const was = JSON.parse(store.getItem('meldframe-reloads') ?? 'null') as { count: number; at: number } | null;
    const count = was && now - was.at <= RELOAD_WINDOW_MS ? was.count : 0;
    if (count >= RELOAD_LIMIT) return false;
    store.setItem('meldframe-reloads', JSON.stringify({ count: count + 1, at: now }));
    return true;
  } catch {
    return false; // no storage, no way to bound the loop: do not reload by itself
  }
}

export function describeSnapshotFailure(status: number, text: string): string {
  const hint = status === 401 ? ' The dev token is missing or wrong: set MELDFRAME_WEB_TOKEN to a token of MELDFRAME_DEV_TOKENS.' : '';
  return `Could not load the file: the server answered ${status}${text ? ` (${text.slice(0, 200)})` : ''}.${hint}`;
}

/** Replace the page with a message (as text, never markup). */
export function showFatal(root: HTMLElement, message: string): void {
  const box = document.createElement('div');
  box.setAttribute('role', 'alert');
  box.dataset.testid = 'fatal';
  box.style.padding = 'var(--space-3)';
  box.style.color = 'var(--colour-danger)';
  box.textContent = message;
  root.replaceChildren(box);
}
