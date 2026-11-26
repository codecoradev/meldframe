// The focus handoff of the React overlays, asked through its interface only (`createHandoff`): where the focus goes when an
// overlay closes, what a menu row or a palette entry that opens a dialog passes on, and that nothing Base UI may call
// twice (`finalFocus`) has a side effect. The wiring to React (the call to `opened` when an overlay opens) is
// `useFocusHandoff.tsx`, which Node cannot run: the browser scripts check it (overlays-e2e, export-e2e, palette-e2e).
import { beforeEach, describe, expect, it } from 'vitest';
import { rememberFocus, takeFocusBack } from './focus';
import { createHandoff } from './handoff';

const el = (connected = true) => ({ isConnected: connected, focused: 0, focus() { this.focused++; } });

beforeEach(() => rememberFocus(null));

describe('an overlay that nothing passed to', () => {
  it('leaves the return to the library: finalFocus answers null, however often it is asked', () => {
    const h = createHandoff();
    h.opened(el());
    expect([h.finalFocus(), h.finalFocus(), h.finalFocus()]).toEqual([null, null, null]);
  });

  it('knows the element that had the focus when it opened', () => {
    const h = createHandoff();
    const opener = el();
    h.opened(opener);
    expect(h.opener).toBe(opener);
    h.opened(null);
    expect(h.opener).toBeNull();
  });
});

describe('a dialog that a menu row passed the focus on to', () => {
  it('answers the menu button, again and again, and never moves the focus itself', () => {
    const button = el();
    const menu = createHandoff();
    menu.opened(button);
    menu.pass(button);
    const dialog = createHandoff();
    dialog.opened(el());
    expect([dialog.finalFocus(), dialog.finalFocus(), dialog.finalFocus()]).toEqual([button, button, button]);
    expect(button.focused).toBe(0);
  });

  it('does not spend what it was given: a second answer after the close is the same', () => {
    const button = el();
    rememberFocus(button);
    const dialog = createHandoff();
    dialog.opened(null);
    expect(dialog.finalFocus()).toBe(button);
    dialog.closed();
    expect(dialog.finalFocus()).toBe(button);
  });

  it('takes the element when it opens, so the next overlay does not inherit it', () => {
    rememberFocus(el());
    createHandoff().opened(null);
    expect(takeFocusBack()).toBeNull();
    const later = createHandoff();
    later.opened(null);
    expect(later.finalFocus()).toBeNull();
  });

  it('falls back to the default when the element left the page before the dialog closed', () => {
    const button = el();
    rememberFocus(button);
    const dialog = createHandoff();
    dialog.opened(null);
    button.isConnected = false;
    expect(dialog.finalFocus()).toBeNull();
  });

  it('forgets it when it opens again: a close with nothing passed is the default', () => {
    const button = el();
    rememberFocus(button);
    const dialog = createHandoff();
    dialog.opened(null);
    dialog.opened(null);
    expect(dialog.finalFocus()).toBeNull();
  });
});

describe('an overlay whose row or entry opens a dialog', () => {
  it('leaves the focus where the dialog takes it (finalFocus answers false) and remembers the place for the dialog', () => {
    const from = el();
    const h = createHandoff();
    h.opened(from);
    h.pass(from);
    expect([h.finalFocus(), h.finalFocus()]).toEqual([false, false]);
    expect(takeFocusBack()).toBe(from);
  });

  it('runs what it was given once, on the close and not in finalFocus', () => {
    const h = createHandoff();
    const ran: string[] = [];
    h.opened(el());
    h.pass(el(), () => ran.push('entry'));
    h.finalFocus();
    h.finalFocus();
    expect(ran).toEqual([]);
    h.closed();
    h.closed();
    expect(ran).toEqual(['entry']);
    expect(h.finalFocus()).toBe(false);
  });

  it('passes the place on to whoever opens next, also when it was given nothing to run', () => {
    const from = el();
    const h = createHandoff();
    h.opened(from);
    h.pass(from);
    h.closed();
    const next = createHandoff();
    next.opened(null);
    expect(next.finalFocus()).toBe(from);
  });

  it('is clean for the next time it opens: the entry is dropped and the close is the default again', () => {
    const h = createHandoff();
    const ran: string[] = [];
    h.opened(el());
    h.pass(el(), () => ran.push('never closed'));
    takeFocusBack(); // the dialog the entry would have opened
    h.opened(el());
    h.closed();
    expect(ran).toEqual([]);
    expect(h.finalFocus()).toBeNull();
  });

  it('passing nothing clears what an earlier pass remembered', () => {
    const h = createHandoff();
    h.opened(el());
    h.pass(el());
    h.pass(null);
    expect(takeFocusBack()).toBeNull();
  });

  it('an entry that throws leaves the handoff clean and raises once', () => {
    const h = createHandoff();
    h.opened(el());
    h.pass(el(), () => {
      throw new Error('entry failed');
    });
    expect(() => h.closed()).toThrow('entry failed');
    expect(() => h.closed()).not.toThrow();
    expect(h.finalFocus()).toBe(false);
  });
});
