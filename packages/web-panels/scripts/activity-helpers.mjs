// Helpers of the Activity scripts (activity-e2e.mjs, checkpoints-e2e.mjs, hostile-names-e2e.mjs): polling, counting the pixels of a
// page image that are (nearly) one of some colours, and the switch to the React copy of the panel (MELDFRAME_UI=react).
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Whether this run asks for the React copy of the Activity panel (`MELDFRAME_UI=react`); the default is the Svelte copy. */
export const react = process.env.MELDFRAME_UI === 'react';

/** The page URL of this run: the same page, with `/?react=<panels>` when the React copy was asked for (the Activity panel by default). */
export const pageUrl = (url, panels = 'activity') => (react ? `${url.replace(/\/?$/, '/')}?react=${panels}` : url);

/** Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none. */
export const fibre = (page, sel) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);

/**
 * Opens `url` with the chunks of the React copy of the Activity panel blocked (the island helper and the panel: the paths of the Vite
 * dev server's modules, `.../react/ActivityPanel.tsx` and `.../react/island.tsx`, so this is for the dev server and not for a production
 * build, whose chunks have hashed names), as a person whose network failed would load it. The page the Svelte copy should then draw.
 */
export async function openWithChunksBlocked(browser, url, pageErrors) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.route((u) => /\/react\/(ActivityPanel|island)\b/.test(u.pathname), (route) => route.abort('failed'));
  await page.goto(url);
  await until('live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  return { ctx, page };
}

/** Polls `fn` until it answers something truthy; fails with `what` after `ms`. */
export async function until(what, fn, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(200);
  }
}

const rgb = (hex) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));

/** How many pixels of `box` (a clip in CSS pixels) are within 8 of one of the `colours` (`#rrggbb`) on every channel. */
export async function rgbCount(page, box, colours) {
  const b64 = (await page.screenshot({ clip: box })).toString('base64');
  return page.evaluate(async ({ b64, want }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let k = 0;
    for (let i = 0; i < d.length; i += 4) if (want.some((w) => Math.abs(d[i] - w[0]) < 9 && Math.abs(d[i + 1] - w[1]) < 9 && Math.abs(d[i + 2] - w[2]) < 9)) k++;
    return k;
  }, { b64, want: colours.map(rgb) });
}
