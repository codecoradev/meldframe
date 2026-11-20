// How the shortcuts are written on this platform: Option, Command and Shift symbols on a Mac, words elsewhere. Pure.

export interface Labels {
  layers: string;
  inspector: string;
  undo: string;
  redo: string;
}

export function isMac(platform: string): boolean {
  return /^(mac|iphone|ipad|ipod)/i.test(platform);
}

export function labels(mac: boolean): Labels {
  return mac
    ? { layers: '⌥1', inspector: '⌥2', undo: '⌘Z', redo: '⇧⌘Z' }
    : { layers: 'Alt+1', inspector: 'Alt+2', undo: 'Ctrl+Z', redo: 'Ctrl+Shift+Z' };
}

/** True in a browser on a Mac (false when there is no navigator, as in a render test). */
export const onMac = (): boolean => typeof navigator !== 'undefined' && isMac(navigator.platform ?? '');

/** The labels of the browser this runs in (the Windows and Linux ones when there is no navigator, as in a render test). */
export function platformLabels(): Labels {
  return labels(onMac());
}
