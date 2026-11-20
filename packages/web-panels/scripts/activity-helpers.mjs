// Helpers of activity-e2e.mjs: polling and counting the pixels of a screenshot that are (nearly) one of some colours.
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
