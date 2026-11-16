import { describe, expect, it } from 'vitest';
import { REFUSED_URL, applyAuth, checkTarget, checkToken, guardRequest, isProxied, resolveProxy } from './proxy-guard';

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

describe('isProxied (the same prefix test as the proxy)', () => {
  it('matches what Vite proxies, including look-alike prefixes, and nothing else', () => {
    for (const u of ['/v1', '/v1/files/x', '/v1x', '/v1?a=1']) expect(isProxied(u)).toBe(true);
    for (const u of ['/', '/v2/files', '/x/v1', '', undefined]) expect(isProxied(u)).toBe(false);
  });
  it('the url a refused upgrade is rewritten to is not proxied', () => {
    expect(isProxied(REFUSED_URL)).toBe(false);
  });
});

describe('Sec-Fetch-Site', () => {
  it('lets same-origin and user-initiated (none) requests through', () => {
    expect(guardRequest({ method: 'GET', ...ok, secFetchSite: 'same-origin' }, PORT)).toBeNull();
    expect(guardRequest({ method: 'GET', host: ok.host, secFetchSite: 'none' }, PORT)).toBeNull();
  });
  it('refuses any other value, with or without an Origin', () => {
    for (const v of ['cross-site', 'same-site', 'bogus', '']) {
      expect(guardRequest({ method: 'GET', ...ok, secFetchSite: v }, PORT)?.status, v).toBe(403);
      expect(guardRequest({ method: 'GET', host: ok.host, secFetchSite: v }, PORT)?.status, v).toBe(403);
    }
  });
});

describe('checkToken', () => {
  it('accepts a normal token and an unset one', () => {
    expect(checkToken('mf_dev_0123456789abcdef')).toBeUndefined();
    expect(checkToken(undefined)).toBeUndefined();
  });
  it('rejects line breaks and characters invalid in a header value, without printing the token', () => {
    for (const t of ['abc\r\nX-Evil: 1', 'abc\ndef', 'a b', 'tab\there', 'caf\u00e9', 'nul\u0000']) {
      const e = checkToken(t);
      expect(e, JSON.stringify(t)).toContain('MELDFRAME_WEB_TOKEN');
      expect(e).not.toContain(t);
    }
  });
});

describe('applyAuth', () => {
  const fake = () => {
    const h = new Map<string, string>([['authorization', 'Bearer from-the-browser']]);
    return { h, setHeader: (k: string, v: string) => void h.set(k, v), removeHeader: (k: string) => void h.delete(k) };
  };
  it('sets the dev token over a browser-supplied header', () => {
    const r = fake();
    applyAuth(r, 'dev-token');
    expect(r.h.get('authorization')).toBe('Bearer dev-token');
  });
  it('strips a browser-supplied Authorization header when there is no token', () => {
    const r = fake();
    applyAuth(r, undefined);
    expect(r.h.has('authorization')).toBe(false);
  });
});

describe('resolveProxy', () => {
  it('uses the default target and treats an empty MELDFRAME_SERVER and token as unset', () => {
    const r = resolveProxy({ MELDFRAME_SERVER: '', MELDFRAME_WEB_TOKEN: '' });
    expect(r.target).toBe('http://127.0.0.1:8080');
    expect(r.token).toBeUndefined();
    expect(r.error).toBeUndefined();
    expect(r.changeOrigin).toBe(false);
  });
  it('reports a remote target without the opt-in, and a bad token, as errors', () => {
    expect(resolveProxy({ MELDFRAME_SERVER: 'http://example.test:8080' }).error).toContain('MELDFRAME_WEB_ALLOW_REMOTE');
    const bad = resolveProxy({ MELDFRAME_WEB_TOKEN: 'a\nb' });
    expect(bad.error).toContain('MELDFRAME_WEB_TOKEN');
    expect(bad.error).not.toContain('a\nb');
  });
  it('lets a remote upstream see its own Host when allowed', () => {
    const r = resolveProxy({ MELDFRAME_SERVER: 'https://dev.example.test', MELDFRAME_WEB_ALLOW_REMOTE: '1' });
    expect(r.error).toBeUndefined();
    expect(r.warning).toContain('dev.example.test');
    expect(r.changeOrigin).toBe(true);
  });
});
