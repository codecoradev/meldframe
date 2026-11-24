import { afterEach, describe, expect, it, vi } from 'vitest';
import { paintBody } from './bodycanvas';

// A node with a gradient or an image is drawn into a 2D canvas, which a test in Node does not have. This one records what is asked of
// it: the alpha of the fill's own colour is on while the fill is drawn and off again before the gradient is (issue 234).
type Call = { op: 'fillRect' | 'drawImage'; style: unknown; alpha: number };

function fakeDocument() {
  const calls: Call[] = [];
  const ctx = {
    globalAlpha: 1,
    fillStyle: '' as unknown,
    scale() {},
    beginPath() {},
    roundRect() {},
    clip() {},
    createLinearGradient: () => ({ addColorStop() {} }),
    createRadialGradient: () => ({ addColorStop() {} }),
    fillRect() { calls.push({ op: 'fillRect', style: ctx.fillStyle, alpha: ctx.globalAlpha }); },
    drawImage() { calls.push({ op: 'drawImage', style: null, alpha: ctx.globalAlpha }); },
  };
  vi.stubGlobal('document', { createElement: () => ({ width: 0, height: 0, getContext: () => ctx }) });
  return calls;
}

afterEach(() => vi.unstubAllGlobals());

const box = { w: 100, h: 60, radius: 0 };
const gradient = { kind: 'linear' as const, angle: 90, stops: [{ pos: 0, color: '#ff0000' }, { pos: 1, color: '#0000ff' }] };

describe('paintBody draws the alpha of the fill colour under the gradient', () => {
  it('the fill is drawn with its alpha and the gradient over it with none of it', () => {
    const calls = fakeDocument();
    paintBody({ ...box, fill: '#00ff00', fillAlpha: 0.5, gradient });
    expect(calls).toHaveLength(2);
    expect(calls[0]).toMatchObject({ op: 'fillRect', style: '#00ff00', alpha: 0.5 });
    expect(calls[1]).toMatchObject({ op: 'fillRect', alpha: 1 });
    expect(calls[1]!.style).not.toBe('#00ff00');
  });

  it('a fully transparent fill is drawn at alpha 0, and the gradient over it is not', () => {
    const calls = fakeDocument();
    paintBody({ ...box, fill: '#000000', fillAlpha: 0, gradient });
    expect(calls.map((c) => c.alpha)).toEqual([0, 1]);
  });

  it('a fill with no alpha given is opaque, and no fill draws no rectangle for it', () => {
    const calls = fakeDocument();
    paintBody({ ...box, fill: '#00ff00', gradient });
    expect(calls[0]).toMatchObject({ style: '#00ff00', alpha: 1 });
    const none = fakeDocument();
    paintBody({ ...box, fill: null, fillAlpha: 0.5, gradient });
    expect(none).toHaveLength(1);
    expect(none[0]!.alpha).toBe(1);
  });

  it('an image over a fill with an alpha is drawn opaque', () => {
    const calls = fakeDocument();
    paintBody({ ...box, fill: '#00ff00', fillAlpha: 0.25, image: { bitmap: { width: 10, height: 10 } as never, fit: 'cover' } });
    expect(calls.map((c) => [c.op, c.alpha])).toEqual([['fillRect', 0.25], ['drawImage', 1]]);
  });
});
