import { describe, expect, it } from 'vitest';
import { checkTarget, guardRequest } from './proxy-guard';

const PORT = 5173;
const ok = { host: `127.0.0.1:${PORT}`, origin: `http://127.0.0.1:${PORT}` };

describe('guardRequest (the dev proxy adds the token, so only this page may use it)', () => {
  it('lets the dev page through', () => {
    expect(guardRequest({ method: 'GET', ...ok }, PORT)).toBeNull();
    expect(guardRequest({ method: 'POST', contentType: 'application/json', ...ok }, PORT)).toBeNull();
    expect(guardRequest({ method: 'GET', host: `localhost:${PORT}` }, PORT)).toBeNull();
    expect(guardRequest({ method: 'GET', upgrade: true, host: `[::1]:${PORT}`, origin: `http://[::1]:${PORT}` }, PORT)).toBeNull();
  });
  it('refuses another origin, host, port or a rebinding name', () => {
    expect(guardRequest({ method: 'GET', host: ok.host, origin: 'https://evil.example' }, PORT)?.status).toBe(403);
    expect(guardRequest({ method: 'GET', host: `127.0.0.1:${PORT}`, origin: 'http://127.0.0.1:9999' }, PORT)?.status).toBe(403);
    expect(guardRequest({ method: 'GET', host: `evil.example:${PORT}` }, PORT)?.status).toBe(403);
    expect(guardRequest({ method: 'GET', host: `192.168.1.5:${PORT}` }, PORT)?.status).toBe(403);
    expect(guardRequest({ method: 'GET', host: '127.0.0.1:1' }, PORT)?.status).toBe(403);
    expect(guardRequest({ method: 'GET' }, PORT)?.status).toBe(403);
  });
  it('requires an Origin for a WebSocket upgrade', () => {
    expect(guardRequest({ method: 'GET', upgrade: true, host: ok.host }, PORT)?.status).toBe(403);
  });
  it('requires JSON for a write, so a form post from another page cannot use the token', () => {
    expect(guardRequest({ method: 'POST', contentType: 'text/plain', ...ok }, PORT)?.status).toBe(415);
    expect(guardRequest({ method: 'POST', ...ok }, PORT)?.status).toBe(415);
  });
});

describe('checkTarget', () => {
  it('accepts a loopback server', () => {
    expect(checkTarget('http://127.0.0.1:8080', undefined)).toEqual({});
    expect(checkTarget('http://localhost:8080', undefined)).toEqual({});
  });
  it('refuses a remote server unless MELDFRAME_WEB_ALLOW_REMOTE=1, then warns', () => {
    expect(checkTarget('https://example.com', undefined).error).toMatch(/MELDFRAME_WEB_ALLOW_REMOTE/);
    expect(checkTarget('https://example.com', '0').error).toBeDefined();
    const allowed = checkTarget('https://example.com', '1');
    expect(allowed.error).toBeUndefined();
    expect(allowed.warning).toMatch(/example\.com/);
  });
  it('refuses a target that is not a URL', () => {
    expect(checkTarget('nonsense', undefined).error).toBeDefined();
  });
});
