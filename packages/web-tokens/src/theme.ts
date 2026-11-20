// The chosen theme: dark by default, remembered in localStorage. Storage can be missing or throw
// (private window, blocked site data), so every access is guarded and the page works without it.

import { THEMES, type Theme } from './tokens';

export const STORAGE_KEY = 'meldframe.theme';

type Reader = Pick<Storage, 'getItem'>;
type Writer = Pick<Storage, 'setItem'>;

export function defaultStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

export function loadTheme(storage: Reader | undefined = defaultStorage()): Theme {
  try {
    const v = storage?.getItem(STORAGE_KEY);
    return THEMES.find((t) => t === v) ?? 'dark';
  } catch {
    return 'dark';
  }
}

export function saveTheme(theme: Theme, storage: Writer | undefined = defaultStorage()): void {
  try {
    storage?.setItem(STORAGE_KEY, theme);
  } catch {
    // Not remembered; the choice still applies to this page.
  }
}

export function applyTheme(theme: Theme, root: Pick<HTMLElement, 'dataset'> = document.documentElement): void {
  root.dataset.theme = theme;
}
