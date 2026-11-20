import { describe, expect, it } from 'vitest';
import { serveCapture, type CaptureDeps, type Req } from './capture-serve';

const req: Req = { requestId: 'r1', nodeId: 'a', scale: 2, maxPixels: 1_000_000, minSeq: 5, uploadToken: 'tok', timeoutMs: 1000 };

function deps(over: Partial<CaptureDeps> = {}) {
  const log: string[] = [];
  const d: CaptureDeps = {
    box: () => ({ x: 0, y: 0, w: 100, h: 50 }),
    render: async (_b, scale) => {
      log.push(`render@${scale}`);
      return new Blob(['png']);
    },
    seq: () => 7,
    whenSeq: async () => true,
    upload: async (id, headers) => {
      log.push(`upload ${id} ${headers['x-capture-seq']} ${headers['x-capture-token']}`);
      return 200;
    },
    fail: (id, code) => log.push(`fail ${id} ${code}`),
    ...over,
  };
  return { d, log };
}

describe('serveCapture', () => {
  it('renders the node and uploads the image with the sequence number it was rendered at', async () => {
    const { d, log } = deps();
    await serveCapture(req, d);
    expect(log).toEqual(['render@2', 'upload r1 7 tok']);
  });
  it('says node_not_found for a node that is not on the canvas, without rendering', async () => {
    const { d, log } = deps({ box: () => null });
    await serveCapture(req, d);
    expect(log).toEqual(['fail r1 node_not_found']);
  });
  it('says too_large for an image over the pixel limit', async () => {
    const { d, log } = deps();
    await serveCapture({ ...req, maxPixels: 10 }, d);
    expect(log).toEqual(['fail r1 too_large']);
  });
  it('says stale_client when the file does not reach the sequence number in time', async () => {
    const { d, log } = deps({ whenSeq: async () => false });
    await serveCapture(req, d);
    expect(log).toEqual(['fail r1 stale_client']);
  });
  it('says so when the drawing fails, and does not upload', async () => {
    const { d, log } = deps({
      render: async () => {
        throw new Error('no gpu');
      },
    });
    await serveCapture(req, d);
    expect(log).toEqual(['fail r1 internal']);
  });
  it('says so when the upload is refused, so the agent does not wait for the timeout', async () => {
    const { d, log } = deps({ upload: async () => 413 });
    await serveCapture(req, d);
    expect(log.at(-1)).toBe('fail r1 internal');
  });
  it('checks the size again on the box it renders, which can have grown while it waited', async () => {
    let calls = 0;
    const { d, log } = deps({ box: () => (++calls === 1 ? { x: 0, y: 0, w: 10, h: 10 } : { x: 0, y: 0, w: 10000, h: 10000 }) });
    await serveCapture(req, d);
    expect(log).toEqual(['fail r1 too_large']);
  });
  it('whole page: no node id asks for the box of all the content', async () => {
    let asked: string | undefined | null = null;
    const { d } = deps({
      box: (id) => {
        asked = id;
        return { x: 0, y: 0, w: 10, h: 10 };
      },
    });
    await serveCapture({ ...req, nodeId: undefined }, d);
    expect(asked).toBeUndefined();
  });
});
