// A PNG for a browser check to upload (image-fill-e2e.mjs, pixels-e2e.mjs): `pngOf(w, h, rowColour)` is an image of w x h pixels in
// which every row has the colour `rowColour(y)` (`#rrggbb`), so a tile that shows it is told from another by its stripes, and
// `uploadPng` sends it to the workspace of a token through POST /v1/assets (the route the upload_asset tool of the agents shares).
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { deflateSync } from 'node:zlib';

/** The three channels of `#rrggbb`. */
export const rgb = (hex) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));

const crcTable = Array.from({ length: 256 }, (_, i) => { let c = i; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = (buf) => { let c = 0xffffffff; for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
/** A PNG of `w` x `h` pixels from `rowColour(y)` (a row is one colour). */
export function pngOf(w, h, rowColour) {
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const sum = Buffer.alloc(4); sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const head = Buffer.alloc(13); head.writeUInt32BE(w, 0); head.writeUInt32BE(h, 4); head[8] = 8; head[9] = 2;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) { const o = y * (w * 3 + 1); raw[o] = 0; const [r, g, b] = rgb(rowColour(y)); for (let x = 0; x < w; x++) { raw[o + 1 + x * 3] = r; raw[o + 2 + x * 3] = g; raw[o + 3 + x * 3] = b; } }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', head), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

/** Uploads an image to the workspace of the token `who` on `server` and returns its id (the SHA-256 of the bytes). */
export async function uploadPng(server, who, bytes) {
  const res = await fetch(`${server}/v1/assets`, { method: 'POST', headers: { authorization: `Bearer ${who}` }, body: bytes });
  const body = await res.json();
  assert.equal(res.status, 200, `upload refused: ${res.status} ${JSON.stringify(body)}`);
  assert.equal(body.id, createHash('sha256').update(bytes).digest('hex'));
  return body.id;
}
