// Candidate contract mapping for model properties crates/export (spike commit ca74052) does NOT emit yet.
// This is experiment code: it appends [data-node] rules to the exported HTML. It is the proposal under test, not shipped code.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { OUT } from './lib.mjs';

const crcT = new Uint32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = (b) => { let c = 0xffffffff; for (const x of b) c = crcT[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
const chunk = (t, d) => { const l = Buffer.alloc(4); l.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
/** 200x100 PNG: left half red, right half blue. */
export function testPng() {
  const w = 200, h = 100, raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const o = y * (w * 3 + 1) + 1 + x * 3; raw[o] = x < 100 ? 255 : 0; raw[o + 2] = x < 100 ? 0 : 255; }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

const px = (v) => `${v}px`;
export function overlayRules(nodes) {
  const css = [];
  for (const n of nodes) {
    const sel = `[data-node="${n.id}"]`;
    const body = [];
    if (n.type === 'text') {
      if (n.fontWeight && n.fontWeight !== 400) body.push(`font-weight:${n.fontWeight}`);
      if (n.textAlign && n.textAlign !== 'left') body.push(`text-align:${n.textAlign}`);
    } else {
      if (n.shadows?.length) {
        // model: first = bottom. CSS box-shadow: first = top. Reverse.
        body.push('box-shadow:' + [...n.shadows].reverse().map((s) => `${px(s.x)} ${px(s.y)} ${px(s.blur)} ${px(s.spread)} ${s.color}`).join(','));
      }
      if (n.image) {
        const size = { cover: 'cover', contain: 'contain', fill: '100% 100%' }[n.image.fit ?? 'cover'];
        body.push(`background-image:url('/out/asset.png')`, `background-size:${size}`, 'background-position:center', 'background-repeat:no-repeat');
      }
      if (n.border?.width > 0) {
        const w = n.border.width, inset = n.border.position === 'inside' ? 0 : n.border.position === 'center' ? -w / 2 : -w;
        const rad = n.type === 'ellipse' ? '50%' : px(Math.min(n.radius, n.w / 2, n.h / 2) + (n.border.position === 'inside' ? 0 : n.border.position === 'center' ? w / 2 : w) * (n.radius > 0 ? 1 : 0));
        body.push('position:relative');
        css.push(`${sel}::after{content:"";position:absolute;pointer-events:none;inset:${px(inset)};border:${px(w)} solid ${n.border.color};border-radius:${rad}}`);
      }
    }
    if (body.length) css.push(`${sel}{${body.join(';')}}`);
  }
  return css.join('\n');
}

export async function overlayHtml(html, nodes) {
  fs.writeFileSync(path.join(OUT, 'asset.png'), testPng());
  const rules = overlayRules(nodes);
  // `position:relative` must not win over position:absolute on root nodes: re-assert absolute roots after the overlay
  const roots = nodes.filter((n) => !n.parentId && n.border?.width > 0).map((n) => `[data-node="${n.id}"]{position:absolute}`).join('\n');
  return html.replace('</style>', `${rules}\n${roots}\n</style>`);
}
