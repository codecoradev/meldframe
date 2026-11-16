// The dev proxy adds the dev token to every `/v1` call, so it must serve the dev page and nobody else: a
// page on another origin (or a rebinding host name) must not be able to use it. Pure, so it is tested.

export interface Req {
  method?: string;
  host?: string;
  origin?: string;
  contentType?: string;
  /** A WebSocket upgrade. */
  upgrade?: boolean;
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
