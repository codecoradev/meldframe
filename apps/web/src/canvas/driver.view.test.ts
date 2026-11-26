// What the driver shows of the scene, with no browser (fake application, fake host, a clock turned by hand): what is culled and when, the
// text sharpened when the view settles, the touch ring that keeps asking for frames while it fades, the images of image fills and the
// capture. Split from driver.test.ts (the 500 lines of a file); the fakes are driver.testutil.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ASSET_RETRY_EVERY_MS, ASSET_TIMEOUT_MS } from './api';
import { WorldView } from './stage';
import { props, rect, scene, setup } from './driver.testutil';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const png = () => ({ toBlob: (cb: (b: Blob | null) => void) => cb(new Blob(['x'], { type: 'image/png' })) }) as unknown as HTMLCanvasElement;

describe('culling', () => {
  const far = scene(rect('near', 0), rect('far', 5000));

  it('draws what is in the view, and a node that comes into the view when the camera moves', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: far }));
    await t.mount();
    t.frame();
    expect(t.driver.stats().nodes).toBe(2);
    expect(t.driver.stats().drawn).toBe(1);
    t.driver.setProps(props({ scene: far, camera: { x: -5000, y: 0, zoom: 1 } }));
    t.frame();
    expect(t.driver.stats().drawn).toBe(1); // the far one now, the near one is out of the view
    t.driver.setProps(props({ scene: far, camera: { x: 145, y: 0, zoom: 0.1 } }));
    t.frame();
    expect(t.driver.stats().drawn).toBe(2);
  });

  it('a node that is moved into the view is culled again and drawn, and one that is added outside it stays out, with the camera unchanged', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: far }));
    await t.mount();
    t.frame();
    expect(t.driver.stats().drawn).toBe(1); // `far` starts outside the view
    t.driver.setProps(props({ scene: scene(rect('near', 0), rect('far', 200)) })); // moved into it
    t.frame();
    expect(t.driver.stats().drawn).toBe(2);
    t.driver.setProps(props({ scene: scene(rect('near', 0), rect('far', 200), rect('added', 9000)) })); // added outside it
    t.frame();
    expect(t.driver.stats().nodes).toBe(3);
    expect(t.driver.stats().drawn).toBe(2);
  });

  it('a camera that the canvas moves to by itself (fit) is culled against its new transform at once, with no frame and no timer', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: far }));
    await t.mount();
    t.frame();
    expect(t.driver.stats().drawn).toBe(1);
    t.driver.fitAll(); // both nodes are in the view now
    expect(t.driver.stats().drawn).toBe(2);
  });
});

describe('sharpening text', () => {
  it('is done when the view has settled, 120 ms after the last move, for the new zoom and the density', async () => {
    const t = setup();
    await t.mount();
    t.frame();
    const sharpen = vi.spyOn(WorldView.prototype, 'sharpenText');
    sharpen.mockClear();
    t.driver.setProps(props({ camera: { x: 10, y: 0, zoom: 2 } }));
    await vi.advanceTimersByTimeAsync(119);
    expect(sharpen).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2);
    expect(sharpen).toHaveBeenCalledWith(2, 1);
    sharpen.mockRestore();
  });
});

describe('a touch ring', () => {
  const touch = (at: number) => ({ id: 't1', color: 0xff0000, label: 'agent', nodeIds: ['missing'], at });

  it('keeps asking for frames while it fades, by itself, and stops when it is gone', async () => {
    const t = setup();
    t.driver.setProps(props({ touches: [touch(performance.now())] }));
    await t.mount();
    t.frame(); // draws the ring
    expect(t.pending()).toBe(1);
    await vi.advanceTimersByTimeAsync(500);
    t.frame();
    expect(t.pending()).toBe(1);
    await vi.advanceTimersByTimeAsync(3000); // past the fade and the hold of reduced motion
    t.frame();
    expect(t.pending()).toBe(0);
  });

  it('asks for no further frame when there is no ring', async () => {
    const t = setup();
    await t.mount();
    t.frame();
    expect(t.pending()).toBe(0);
  });
});

describe('the images of image fills', () => {
  const ID = 'a'.repeat(64);
  const pictured = () => scene({ ...rect('p', 0), image: { assetId: ID, fit: 'cover' as const } });
  const missing = { ok: false, status: 404, blob: async () => new Blob(), json: async () => ({ error: { code: 'asset_not_found' } }) };

  it('tells the host about an image that is missing, with its id and the code', async () => {
    const t = setup();
    const onAssetProblem = vi.fn();
    t.driver.events = { ...t.events, onAssetProblem };
    t.platform.assets = { fetch: async () => missing, decode: async () => ({ width: 1, height: 1 }) };
    t.driver.setProps(props({ scene: pictured() }));
    await t.mount();
    await vi.advanceTimersByTimeAsync(10);
    expect(onAssetProblem).toHaveBeenCalledTimes(1);
    expect(onAssetProblem).toHaveBeenCalledWith(ID, 'asset_not_found');
  });

  it('a capture waits for the pictures the view asked for, and not for ever', async () => {
    const holds = async (release: boolean) => {
      const t = setup();
      let answer!: () => void;
      t.platform.assets = { fetch: () => new Promise((r) => (answer = () => r(missing))), decode: async () => ({ width: 1, height: 1 }) };
      t.driver.setProps(props({ scene: pictured() }));
      await t.mount();
      t.frame();
      const region = vi.spyOn(WorldView.prototype, 'renderRegion').mockReturnValue(png());
      const done = t.driver.capture({ x: 0, y: 0, w: 10, h: 10 }, 1);
      await vi.advanceTimersByTimeAsync(1000);
      const early = region.mock.calls.length;
      if (release) answer();
      await vi.advanceTimersByTimeAsync(release ? 0 : ASSET_TIMEOUT_MS);
      await done;
      const late = region.mock.calls.length;
      region.mockRestore();
      return { early, late };
    };
    expect(await holds(true)).toEqual({ early: 0, late: 1 }); // drawn once the picture has answered
    expect(await holds(false)).toEqual({ early: 0, late: 1 }); // drawn after the limit, by the fill
  });
});

describe('capture', () => {
  it('says the canvas is not ready before it has started and after the cleanup', async () => {
    const t = setup();
    await expect(t.driver.capture({ x: 0, y: 0, w: 10, h: 10 }, 1)).rejects.toThrow('The canvas is not ready.');
    const dispose = await t.mount();
    dispose();
    await expect(t.driver.capture({ x: 0, y: 0, w: 10, h: 10 }, 1)).rejects.toThrow('The canvas is not ready.');
  });

  it('draws the region at the scale, hands over the PNG, and culls again for the view afterwards', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('a', 0)) }));
    await t.mount();
    t.frame();
    const image = { toBlob: (cb: (b: Blob | null) => void) => cb(new Blob(['x'], { type: 'image/png' })) } as unknown as HTMLCanvasElement;
    const region = vi.spyOn(WorldView.prototype, 'renderRegion').mockReturnValue(image);
    const blob = await t.driver.capture({ x: 0, y: 0, w: 10.2, h: 5 }, 2);
    expect(blob.type).toBe('image/png');
    expect(region).toHaveBeenCalledWith(t.app.renderer, { x: 0, y: 0, w: 10.2, h: 5 }, 2, 21, 10);
    expect(t.pending()).toBe(1);
    region.mockRestore();
  });

  it('culls again for the view afterwards: what the drawing showed that the view does not is hidden again', async () => {
    const t = setup();
    t.driver.setProps(props({ scene: scene(rect('near', 0), rect('far', 5000)) }));
    await t.mount();
    t.frame();
    expect(t.driver.stats().drawn).toBe(1);
    // The drawing of a region shows everything (it draws the world translated to the box).
    const region = vi.spyOn(WorldView.prototype, 'renderRegion').mockImplementation(function (this: WorldView) {
      for (const c of this.root.children) c.culled = false;
      return png();
    });
    await t.driver.capture({ x: 0, y: 0, w: 10, h: 10 }, 1);
    expect(t.driver.stats().drawn).toBe(2);
    t.frame();
    expect(t.driver.stats().drawn).toBe(1);
    region.mockRestore();
  });

  it('gives the box of a node, or of all the content, with the band of a border', async () => {
    const t = setup();
    // `paintBounds` is what the render step works out (an outside border of 6 here); the driver asks the scene and adds nothing.
    t.driver.setProps(props({ scene: scene(rect('a', 0), { ...rect('b', 300), paintBounds: { left: 6, top: 6, right: 6, bottom: 6 } }) }));
    await t.mount();
    expect(t.driver.captureBox('a')).toEqual({ x: 0, y: 0, w: 100, h: 100 });
    expect(t.driver.captureBox('b')).toEqual({ x: 294, y: -6, w: 112, h: 112 });
    expect(t.driver.captureBox()).toEqual({ x: 0, y: -6, w: 406, h: 112 });
    expect(t.driver.captureBox('missing')).toBeNull();
  });
});

describe('the asset retry timer', () => {
  it('runs while the canvas is mounted and stops with it', async () => {
    const t = setup();
    const dispose = await t.mount();
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    await vi.advanceTimersByTimeAsync(ASSET_RETRY_EVERY_MS * 2);
    dispose();
    expect(vi.getTimerCount()).toBe(0);
  });
});
