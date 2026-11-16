import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const protocol = fileURLToPath(new URL('../../packages/protocol/src/index.ts', import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  resolve: { alias: { '@meldframe/protocol': protocol }, conditions: ['browser'] },
  test: { include: ['src/**/*.test.ts', 'scripts/**/*.test.mjs'], environment: 'node' },
});
