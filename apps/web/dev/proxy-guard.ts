// The dev proxy adds the dev token to every `/v1` call, so it must serve the dev page and nobody else: a
// page on another origin (or a rebinding host name) must not be able to use it. Pure, so it is tested.

export interface Req {
  method?: string;
  host?: string;
  origin?: string;
  contentType?: string;
  /** A WebSocket upgrade. */
  upgrade?: boolean;
  /** The `Sec-Fetch-Site` header, when the browser sent one. */
  secFetchSite?: string;
}

const LOOPBACK = new Set(['localhost', '127.0.0.1', '[::1]']);

function parseHost(h: string): { name: string; port: string } | null {
  const m = /^(\[[^\]]+\]|[^:]+)(?::(\d+))?$/.exec(h);
  return m ? { name: m[1]!.toLowerCase(), port: m[2] ?? '' } : null;
}

/** null to let the request through, else the status and reason to answer with. */
export function guardRequest(r: Req, port: number): { status: number; reason: string } | null {
  const host = r.host ? parseHost(r.host) : null;
  if (!host || !LOOPBACK.has(host.name) || host.port !== String(port)) {
    return { status: 403, reason: 'the dev proxy answers only to its own loopback address' };
  }
  // A browser says where a request comes from; only the page itself (same-origin) or the user typing the address
  // (none) may use the proxy, whether or not an Origin header came with it.
  if (r.secFetchSite !== undefined && r.secFetchSite !== 'same-origin' && r.secFetchSite !== 'none') {
    return { status: 403, reason: 'the request does not come from the dev page' };
  }
  if (r.origin !== undefined) {
    if (r.origin !== `http://${r.host}`) return { status: 403, reason: 'the origin is not the dev page' };
  } else if (r.upgrade) {
    return { status: 403, reason: 'a WebSocket needs the dev page as its origin' };
  }
  const reads = r.method === undefined || r.method === 'GET' || r.method === 'HEAD';
  if (!reads && !(r.contentType ?? '').toLowerCase().startsWith('application/json')) {
    return { status: 415, reason: 'writes must be application/json' };
  }
  return null;
}

/** The server the proxy forwards to, with the token: loopback only, unless MELDFRAME_WEB_ALLOW_REMOTE=1. */
export function checkTarget(target: string, allowRemote: string | undefined): { error?: string; warning?: string } {
  let url: URL;
  try {
    url = new URL(target);
  } catch {
    return { error: `MELDFRAME_SERVER is not a URL: ${target}` };
  }
  if (LOOPBACK.has(url.hostname)) return {};
  if (allowRemote !== '1') {
    return { error: `MELDFRAME_SERVER ${url.host} is not on this machine; the proxy would send the dev token there. Set MELDFRAME_WEB_ALLOW_REMOTE=1 to allow it.` };
  }
  return { warning: `the dev proxy sends the dev token to ${url.host}, which is not on this machine` };
}

/** The prefix test of the proxy (Vite proxies every url that starts with the context); the guard uses the same one. */
export const PROXY_PREFIX = '/v1';
export const isProxied = (url: string | undefined): boolean => url !== undefined && url.startsWith(PROXY_PREFIX);

/** What a refused upgrade's url is rewritten to, so the proxy's own upgrade listener does not connect upstream. */
export const REFUSED_URL = '/__refused';

/** An error for a token that cannot be a header value (never includes the token), else undefined. */
export function checkToken(token: string | undefined): string | undefined {
  if (token === undefined || /^[\x21-\x7e]+$/.test(token)) return undefined;
  return 'MELDFRAME_WEB_TOKEN holds a character that is not allowed in a header value (a line break, a space or a non-ASCII character); check the token';
}

interface AuthTarget {
  setHeader(k: string, v: string): void;
  removeHeader(k: string): void;
}

/** The proxy owns the Authorization header: the dev token when there is one, else none (never the browser's own). */
export function applyAuth(req: AuthTarget, token: string | undefined): void {
  if (token) req.setHeader('authorization', `Bearer ${token}`);
  else req.removeHeader('authorization');
}

export const DEFAULT_TARGET = 'http://127.0.0.1:8080';

/** What the dev proxy runs with, from the environment. An empty variable counts as unset. */
export function resolveProxy(env: Record<string, string | undefined>): {
  target: string;
  token: string | undefined;
  changeOrigin: boolean;
  error?: string;
  warning?: string;
} {
  const target = env.MELDFRAME_SERVER?.trim() || DEFAULT_TARGET;
  const token = env.MELDFRAME_WEB_TOKEN || undefined;
  const checked = checkTarget(target, env.MELDFRAME_WEB_ALLOW_REMOTE);
  let remote = false;
  try {
    remote = !LOOPBACK.has(new URL(target).hostname);
  } catch {
    // checkTarget already reports it.
  }
  return { target, token, changeOrigin: remote, error: checked.error ?? checkToken(token), warning: checked.warning };
}
