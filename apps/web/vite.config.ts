import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import type { IncomingMessage } from 'node:http';
import type { AddressInfo } from 'node:net';
import type { Duplex } from 'node:stream';
import type { Plugin, PreviewServer, ViteDevServer } from 'vite';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { REFUSED_URL, applyAuth, guardRequest, isProxied, resolveProxy } from '../../packages/web-tools/src/proxy-guard';

// Development only. The browser never holds a token: this server-side proxy adds the dev token to the
// REST calls and the live socket (a browser WebSocket cannot set a header). Set these in the
// environment, never in a committed file: MELDFRAME_WEB_TOKEN (a token of MELDFRAME_DEV_TOKENS; `just web`
// reads it from .dev-token) and MELDFRAME_SERVER (loopback unless MELDFRAME_WEB_ALLOW_REMOTE=1). The proxy
// answers only to the dev page on its own loopback address (see packages/web-tools/src/proxy-guard.ts), so `vite --host`
// does not hand the token to the network.
const wasmGlue = existsSync(new URL('./.wasm/meldframe_wasm.js', import.meta.url)) ? './.wasm/meldframe_wasm.js' : './src/wasm-missing.ts';
const webPackages = Object.fromEntries(
  ['canvas', 'input', 'assets', 'link', 'transport', 'document', 'tokens', 'chrome', 'shell', 'panels', 'variables', 'variables-io', 'activity', 'feedback', 'export'].map((p) => [`@meldframe/web-${p}`, fileURLToPath(new URL(`../../packages/web-${p}/src`, import.meta.url))]),
);
const resolved = resolveProxy(process.env);
const { target, token } = resolved;

const describe = (req: IncomingMessage, upgrade: boolean) => ({
  method: req.method,
  host: req.headers.host,
  origin: req.headers.origin,
  contentType: req.headers['content-type'],
  upgrade,
  secFetchSite: req.headers['sec-fetch-site'] as string | undefined,
  url: req.url,
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

type ProxyReq = { setHeader(k: string, v: string): void; removeHeader(k: string): void };
const route = {
  target,
  ws: true,
  // A remote upstream (opt-in) must see its own Host, not the dev page's.
  changeOrigin: resolved.changeOrigin,
  configure(p: { on(event: string, fn: (req: ProxyReq) => void): void }) {
    const add = (req: ProxyReq) => applyAuth(req, token);
    p.on('proxyReq', add);
    p.on('proxyReqWs', add);
  },
};
// `/captures` is the editor's PNG upload for a capture request of an agent (it needs the same login).
const proxy = { '/v1': route, '^/captures(/|$)': route };

export default defineConfig(({ command }) => {
  // Only a running server (dev or preview) refuses a bad target or token; `vite build` and the tests do not need them.
  if (command === 'serve' && !process.env.VITEST) {
    if (resolved.error) throw new Error(resolved.error);
    if (resolved.warning) console.warn(`warning: ${resolved.warning}`);
  }
  return {
    plugins: [svelte(), tailwindcss(), guard()],
    // The file to open and the actor of this client's ops (development: the dev token's file and actor).
    define: {
      __FILE__: JSON.stringify(process.env.MELDFRAME_WEB_FILE ?? 'file_dev'),
      __ACTOR__: JSON.stringify(process.env.MELDFRAME_WEB_ACTOR ?? 'agent:dev'),
    },
    resolve: {
      conditions: ['browser'],
      // The web packages (packages/web-*, each its own unit of the size check) are used as source.
      alias: {
        ...webPackages,
        'bits-ui': fileURLToPath(new URL('./node_modules/bits-ui', import.meta.url)),
        // The generated WASM glue (tools/build-wasm.sh). Without it the stub stands in, which fails with a clear
        // message when the page starts, so the build and the type check run without the wasm toolchain.
        '@meldframe/wasm': fileURLToPath(new URL(wasmGlue, import.meta.url)),
      },
    },
    server: { proxy },
    preview: { proxy },
    test: {
      include: ['src/**/*.test.ts', '../../packages/web-*/src/**/*.test.ts', '../../packages/web-*/scripts/**/*.test.mjs'],
      environment: 'node',
    },
  };
});
