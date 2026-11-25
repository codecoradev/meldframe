import { describe, expect, it } from 'vitest';
import { LoadFailure } from '@meldframe/web-document/file';
import { pageFor } from './failure';

const refused = (status: number, body = '') => new LoadFailure(status, body);
// The helper of the app layer is Rust (`displayText`, tested in crates/app/meldframe-wasm/tests/display_text.rs); here a stand-in
// that records how it was called and shows its answer, so the test says what this file passes and what it draws.
const calls: [string, number][] = [];
const safe = (text: string, max: number) => (calls.push([text, max]), text.slice(0, max));
const product = { dev: false, safe };
const dev = { dev: true, safe };

describe('the page for a file that did not open', () => {
  it('403 says there is no access and that nothing changed, with what the server answered as detail', () => {
    const p = pageFor(refused(403, 'read policy says no'), product);
    expect(p).toMatchObject({ kind: 'no-access', title: 'No access to this file' });
    expect(p.text).toMatch(/nothing was changed/i);
    expect(p.detail).toBe('The server answered 403: read policy says no');
    expect(p.action).toBe('Try again');
  });

  it('401 says the server did not accept the page, promises no sign-in, and shows no developer hint to a person', () => {
    const p = pageFor(refused(401, 'no'), product);
    expect(p).toMatchObject({ kind: 'not-accepted' });
    expect(p.title).toMatch(/did not accept/i);
    expect(`${p.title} ${p.text}`).not.toMatch(/sign in/i);
    expect(p.detail).toBe('The server answered 401: no');
    expect(p.detail).not.toMatch(/just dev|just web|dev token/i);
  });

  it('the developer hint for a missing token is shown in a development build only', () => {
    expect(pageFor(refused(401), dev).detail).toContain('just dev');
    expect(pageFor(refused(401), product).detail ?? '').not.toContain('just dev');
    expect(pageFor(refused(500, 'boom'), dev).detail).not.toContain('just dev');
  });

  it('a request with no answer (status 0) says the server cannot be reached, without claiming edits were kept', () => {
    const p = pageFor(refused(0, 'Failed to fetch'), product);
    expect(p).toMatchObject({ kind: 'unreachable', title: "Can't reach the server" });
    expect(p.text).not.toMatch(/kept on this device/i);
    expect(p.action).toBe('Try again');
  });

  it('a TypeError that did not come from the snapshot request is a failure of the page, with its message as plain text', () => {
    const p = pageFor(new TypeError("Cannot read properties of undefined (reading 'x')"), product);
    expect(p).toMatchObject({ kind: 'start', title: 'The editor could not start' });
    expect(p.detail).toBe("Cannot read properties of undefined (reading 'x')");
    expect(pageFor(new TypeError('Failed to fetch'), product).kind).toBe('start'); // the WASM module failing to load
  });

  it('any other answer is the server failing to open the file, with its status in the detail', () => {
    const p = pageFor(refused(500, 'boom'), product);
    expect(p).toMatchObject({ kind: 'server' });
    expect(p.detail).toBe('The server answered 500: boom');
  });

  it('anything else is the editor failing to start, with its message', () => {
    expect(pageFor(new Error('WebAssembly is not available'), product).detail).toBe('WebAssembly is not available');
    expect(pageFor('plain string', product).detail).toBe('plain string');
  });

  it('draws the text of the server and of the browser as the helper of the app layer answers, cut at 200 and 500 characters', () => {
    calls.length = 0;
    const out = pageFor(refused(500, 'x'.repeat(300)), product);
    expect(calls).toEqual([['x'.repeat(300), 200]]);
    expect(out.detail).toBe(`The server answered 500: ${'x'.repeat(200)}`);
    calls.length = 0;
    pageFor(new Error('y'.repeat(900)), product);
    expect(calls).toEqual([['y'.repeat(900), 500]]);
  });

  it('cleans nothing itself: whatever the helper hands back is what is shown', () => {
    const marked = { dev: false, safe: () => 'MARKED' };
    expect(pageFor(refused(500, 'a\u202eb'), marked).detail).toBe('The server answered 500: MARKED');
    expect(pageFor(new Error('x\u202ey'), marked).detail).toBe('MARKED');
  });

  it('shows no text of the server or of the browser when the helper is not there (the WASM module did not load)', () => {
    const none = { dev: false, safe: null };
    expect(pageFor(refused(500, 'a\u202eb secret body'), none).detail).toBe('The server answered 500.');
    expect(pageFor(refused(403, 'no'), none).detail).toBe('The server answered 403.');
    const start = pageFor(new Error('x\u202ey'), none);
    expect(start).toMatchObject({ kind: 'start', title: 'The editor could not start' });
    expect(start.detail).toBeUndefined();
  });

  it('uses the words of GLOSSARY.md in the text and the detail: no account, no token, no team', () => {
    for (const e of [refused(403, 'no'), refused(401, 'no'), refused(500, 'x'), refused(0, 'Failed'), new TypeError('x'), new Error('y')]) {
      const p = pageFor(e, product);
      expect(`${p.title} ${p.text} ${p.detail ?? ''}`).not.toMatch(/\b(account|token|team|organi[sz]ation)\b/i);
    }
  });
});
