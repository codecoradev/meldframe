// The product switch "Auto layout on canvas": whether the canvas draws laid-out boxes (hug, fill, auto layout) or the stored ones.
// The rules are the app layer's (`App::set_layout_on_canvas`, `docs/architecture.md`); this is the host half: where the choice
// comes from (the address, then what the person chose in this browser, then the default, on), where it is remembered, and that
// the measurer that sizes the texts is installed before the first laid-out scene is drawn. The storage and the measurer are
// injected, so the choice is a test with no browser.

/** Where the person's choice is kept in this browser (`on` or `off`). */
export const LAYOUT_STORAGE_KEY = 'meldframe.autoLayoutOnCanvas';

/**
 * Whether the canvas starts laid out: `?canvaslayout=1` or `?canvaslayout=0` in the address forces it for this page (and is not
 * remembered); else the choice remembered in this browser; else on. A storage that cannot be had or read counts as no choice.
 */
export function layoutChoice(search: string, storage: () => Storage | null): boolean {
  const forced = new URLSearchParams(search).get('canvaslayout');
  if (forced === '1') return true;
  if (forced === '0') return false;
  try {
    return storage()?.getItem(LAYOUT_STORAGE_KEY) !== 'off';
  } catch {
    return true;
  }
}

export interface LayoutSwitch {
  /** Whether the canvas draws laid-out boxes now. */
  readonly on: boolean;
  /** Turn it on or off, remember the choice, and return the new state. */
  toggle(): boolean;
}

/**
 * The switch over an engine. `installMeasurer` gives the engine the measurer of the page's fonts (it is called once, before the
 * first time the flag goes on, so the first laid-out scene is already measured with real fonts). `initial` is where the page
 * starts; it is not remembered, only a change the person makes is.
 */
export function createLayoutSwitch(
  engine: { setLayoutOnCanvas(on: boolean): unknown },
  installMeasurer: () => unknown,
  initial: boolean,
  storage: () => Storage | null,
): LayoutSwitch {
  let installed = false;
  let on = false;
  const set = (value: boolean) => {
    if (value && !installed) {
      installed = true;
      installMeasurer();
    }
    on = value;
    engine.setLayoutOnCanvas(value);
  };
  if (initial) set(true);
  return {
    get on() {
      return on;
    },
    toggle() {
      set(!on);
      try {
        storage()?.setItem(LAYOUT_STORAGE_KEY, on ? 'on' : 'off');
      } catch {
        // No storage: the choice holds for this page only.
      }
      return on;
    },
  };
}

/** What the page says when the switch changes (a live region: the person who cannot see the canvas change hears it). */
export const layoutNotice = (on: boolean): string => `Auto layout on canvas: ${on ? 'on' : 'off'}`;

/** Turn the switch and say what it is now; returns the new state. */
export function toggleWithNotice(sw: LayoutSwitch, notify: (text: string) => void): boolean {
  const on = sw.toggle();
  notify(layoutNotice(on));
  return on;
}
