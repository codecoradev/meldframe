import { svelte } from '@sveltejs/vite-plugin-svelte';
import type { IncomingMessage } from 'node:http';
import type { AddressInfo } from 'node:net';
import type { Duplex } from 'node:stream';
import { fileURLToPath } from 'node:url';
import type { Plugin, PreviewServer, ViteDevServer } from 'vite';
import { defineConfig } from 'vitest/config';
import { REFUSED_URL, checkTarget, guardRequest, isProxied } from './dev/proxy-guard';

const protocol = fileURLToPath(new URL('../../packages/protocol/src/index.ts', import.meta.url));

// Development shell only. The browser never holds a token: this server-side proxy adds the dev token to
// the REST calls and the live socket (a browser WebSocket cannot set a header). Set these in the
// environment, never in a committed file: MELDFRAME_WEB_TOKEN (a token of MELDFRAME_DEV_TOKENS),
// MELDFRAME_WEB_ACTOR (the actor that token stands for), MELDFRAME_WEB_FILE, MELDFRAME_SERVER (loopback
// unless MELDFRAME_WEB_ALLOW_REMOTE=1). The proxy answers only to the dev page on its own loopback
// address (see dev/proxy-guard.ts), so `vite --host` does not hand the token to the network.
const token = process.env.MELDFRAME_WEB_TOKEN;
const target = process.env.MELDFRAME_SERVER ?? 'http://127.0.0.1:8080';
const checked = checkTarget(target, process.env.MELDFRAME_WEB_ALLOW_REMOTE);
if (checked.error) throw new Error(checked.error);
if (checked.warning) console.warn(`warning: ${checked.warning}`);

const describe = (req: IncomingMessage, upgrade: boolean) => ({
  method: req.method,
  host: req.headers.host,
  origin: req.headers.origin,
  contentType: req.headers['content-type'],
  upgrade,
});

/** Runs before the proxy, for HTTP and for WebSocket upgrades of `/v1`. */
function guard(): Plugin {
  const install = (server: ViteDevServer | PreviewServer) => {
    const port = () => (server.httpServer?.address() as AddressInfo | null)?.port ?? 0;
    server.middlewares.use((req, res, next) => {
      if (!isProxied(req.url)) return next();
      const refused = guardRequest(describe(req, false), port());
      if (!refused) return next();
      res.statusCode = refused.status;
      res.end(refused.reason);
    });
    server.httpServer?.prependListener('upgrade', (req: IncomingMessage, socket: Duplex) => {
      if (isProxied(req.url) && guardRequest(describe(req, true), port())) {
        // Vite's own upgrade listener runs next and would connect upstream with the token: hide the url from it.
        req.url = REFUSED_URL;
        socket.destroy();
      }
    });
  };
  return { name: 'meldframe-proxy-guard', configureServer: install, configurePreviewServer: install };
}

const proxy = {
  '/v1': {
    target,
    ws: true,
    configure(p: { on(event: string, fn: (req: { setHeader(k: string, v: string): void }) => void): void }) {
      const add = (req: { setHeader(k: string, v: string): void }) => token && req.setHeader('authorization', `Bearer ${token}`);
      p.on('proxyReq', add);
      p.on('proxyReqWs', add);
    },
  },
};

export default defineConfig({
  plugins: [svelte(), guard()],
  resolve: { alias: { '@meldframe/protocol': protocol }, conditions: ['browser'] },
  define: {
    __ACTOR__: JSON.stringify(process.env.MELDFRAME_WEB_ACTOR ?? 'usr_web'),
    __FILE__: JSON.stringify(process.env.MELDFRAME_WEB_FILE ?? 'file_dev'),
  },
  server: { proxy },
  preview: { proxy },
  test: { include: ['src/**/*.test.ts', 'dev/**/*.test.ts', 'scripts/**/*.test.mjs'], environment: 'node' },
});
