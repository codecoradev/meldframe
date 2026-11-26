// Helpers of the pixel comparison scripts (pixels-e2e.mjs, pixels-activity-e2e.mjs): whether React drew an element, and the pixel by
// pixel comparison of two images with a tolerance of 0.

/** Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none. */
export const fibre = (page, sel) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);

/**
 * A function that compares two images (base64 PNGs) in a page of `browser`: `{ size }` when the sizes differ, else the number of
 * pixels with any channel (alpha included) that differs, the total, and the box the differing pixels lie in (null when none).
 */
export async function makeDiffer(browser) {
  const cmp = await (await browser.newContext()).newPage();
  await cmp.setContent('<canvas></canvas>');
  return (a, b) =>
    cmp.evaluate(async ({ a, b }) => {
      const load = async (b64) => {
        const im = new Image();
        im.src = `data:image/png;base64,${b64}`;
        await im.decode();
        const c = document.createElement('canvas');
        c.width = im.width;
        c.height = im.height;
        const g = c.getContext('2d');
        g.drawImage(im, 0, 0);
        return g.getImageData(0, 0, c.width, c.height);
      };
      const [p, q] = [await load(a), await load(b)];
      if (p.width !== q.width || p.height !== q.height) return { size: [p.width, p.height, q.width, q.height] };
      let k = 0;
      let box = null; // where the differing pixels are, in device pixels: [left, top, right, bottom]
      for (let i = 0; i < p.data.length; i += 4) {
        if (p.data[i] === q.data[i] && p.data[i + 1] === q.data[i + 1] && p.data[i + 2] === q.data[i + 2] && p.data[i + 3] === q.data[i + 3]) continue;
        k++;
        const [x, y] = [(i / 4) % p.width, Math.floor(i / 4 / p.width)];
        box = box ? [Math.min(box[0], x), Math.min(box[1], y), Math.max(box[2], x), Math.max(box[3], y)] : [x, y, x, y];
      }
      return { differing: k, total: p.width * p.height, box };
    }, { a, b });
}
