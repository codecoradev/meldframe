import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const protocol = fileURLToPath(new URL('../../packages/protocol/src/index.ts', import.meta.url));

// Development shell only. The browser never holds a token: this server-side proxy adds the dev token to
// the REST calls and the live socket (a browser WebSocket cannot set a header). Set these in the
// environment, never in a committed file: MELDFRAME_WEB_TOKEN (a token of MELDFRAME_DEV_TOKENS),
// MELDFRAME_WEB_ACTOR (the actor that token stands for), MELDFRAME_WEB_FILE, MELDFRAME_SERVER.
const token = process.env.MELDFRAME_WEB_TOKEN;
const proxy = {
  '/v1': {
    target: process.env.MELDFRAME_SERVER ?? 'http://127.0.0.1:8080',
    ws: true,
    configure(p: { on(event: string, fn: (req: { setHeader(k: string, v: string): void }) => void): void }) {
      const add = (req: { setHeader(k: string, v: string): void }) => token && req.setHeader('authorization', `Bearer ${token}`);
      p.on('proxyReq', add);
      p.on('proxyReqWs', add);
    },
  },
};

export default defineConfig({
  plugins: [svelte()],
  resolve: { alias: { '@meldframe/protocol': protocol }, conditions: ['browser'] },
  define: {
    __ACTOR__: JSON.stringify(process.env.MELDFRAME_WEB_ACTOR ?? 'usr_web'),
    __FILE__: JSON.stringify(process.env.MELDFRAME_WEB_FILE ?? 'file_dev'),
  },
  server: { proxy },
  preview: { proxy },
  test: { include: ['src/**/*.test.ts', 'scripts/**/*.test.mjs'], environment: 'node'},
});
