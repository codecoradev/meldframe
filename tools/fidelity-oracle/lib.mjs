// Shared parts of the fidelity oracle: the runner, a static server, Chromium, the text measurement page, Tailwind.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { stopUnlessGatePlatform } from './platform.mjs';

const require = createRequire(import.meta.url);
export const HERE = path.dirname(fileURLToPath(import.meta.url));
export const ROOT = path.resolve(HERE, '../..');
export const OUT = path.join(HERE, 'out');
const NM = path.join(HERE, 'node_modules');
const FONT = path.join(NM, '@fontsource-variable/inter/files/inter-latin-wght-normal.woff2');
const PIXI = path.join(NM, 'pixi.js/dist/pixi.min.js');
/** The runner (`cargo build -p meldframe-oracle-runner`); ORACLE_RUNNER overrides the path. */
export const RUNNER = process.env.ORACLE_RUNNER ?? path.join(process.env.CARGO_TARGET_DIR ?? path.join(ROOT, 'target'), 'debug/oracle-runner');

// ---- a 200x100 test image, left half red and right half blue (the asset every image fill refers to) ----
const crcT = new Uint32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = (b) => { let c = 0xffffffff; for (const x of b) c = crcT[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
const chunk = (t, d) => { const l = Buffer.alloc(4); l.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
function testPng() {
  const w = 200, h = 100, raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const o = y * (w * 3 + 1) + 1 + x * 3; raw[o] = x < 100 ? 255 : 0; raw[o + 2] = x < 100 ? 0 : 255; }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
const PNG = testPng();
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2', '.png': 'image/png' };

/** Serves out/ and, wherever a page asks for them: the font (`inter.woff2`), the test image (`assets/<64 hex>`) and Pixi. */
export function serve() {
  fs.mkdirSync(OUT, { recursive: true });
  const server = http.createServer((req, res) => {
    let u;
    try { u = decodeURIComponent((req.url ?? '/').split('?')[0]); } catch { return res.writeHead(400).end('bad url'); }
    const send = (body, type) => res.writeHead(200, { 'content-type': type }).end(body);
    if (u.endsWith('/inter.woff2')) return send(fs.readFileSync(FONT), MIME['.woff2']);
    if (/\/assets\/[0-9a-f]{64}$/.test(u)) return send(PNG, MIME['.png']);
    if (u === '/vendor/pixi.min.js') return send(fs.readFileSync(PIXI), MIME['.js']);
    const p = u.startsWith('/out/') ? path.join(OUT, u.slice(5)) : null;
    if (!p || !p.startsWith(OUT + path.sep) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) return res.writeHead(404).end('not found');
    send(fs.readFileSync(p), MIME[path.extname(p)] ?? 'application/octet-stream');
  });
  return new Promise((r) => server.listen(0, '127.0.0.1', () => r({ base: `http://127.0.0.1:${server.address().port}`, close: () => server.close() })));
}

/** Chromium through the pinned Playwright. The browser build is the one Playwright 1.63.0 installs (`npx playwright install chromium`).
 *  On a platform the gate is not defined on (`platform.mjs`) it stops the run with exit code 2 and says why. */
export async function launch() {
  stopUnlessGatePlatform();
  return require('playwright').chromium.launch();
}

const ASSET = 'a'.repeat(64);
export { ASSET };

// ---- text: the table of measurements the runner replays ----
const MEASURE_HTML = `<!doctype html><meta charset=utf-8><style>
@font-face{font-family:'Inter Variable';src:url('/inter.woff2') format('woff2');font-weight:100 900;font-display:block}
body{margin:0;font-family:'Inter Variable'}</style><span>Inter</span>
<script src="/vendor/pixi.min.js"></script>`;

/** The measurement page: Pixi `CanvasTextMetrics` with Inter, as the canvas draws text (apps/web/src/canvas/stage.ts). */
export async function openMeasurePage(browser, base) {
  fs.writeFileSync(path.join(OUT, 'measure.html'), MEASURE_HTML);
  const page = await (await browser.newContext()).newPage();
  await page.goto(`${base}/out/measure.html`);
  await page.evaluate(async () => {
    for (const w of [100, 200, 300, 400, 500, 600, 700, 800, 900]) await document.fonts.load(`${w} 16px "Inter Variable"`);
    await document.fonts.ready;
  });
  if (!(await page.evaluate(() => document.fonts.check('16px "Inter Variable"')))) throw new Error('Inter did not load in the measurement page');
  return {
    page,
    /** [{text, fontSize, fontWeight, wrapW}] to [{..., w, h}] with the width and the height rounded up, as the core's measurer seam expects. */
    measure: (queries) => page.evaluate((qs) => {
      const { CanvasTextMetrics, TextStyle } = window.PIXI;
      return qs.map((q) => {
        const style = new TextStyle({ fontFamily: 'Inter Variable, system-ui, sans-serif', fontSize: q.fontSize, fontWeight: String(q.fontWeight), wordWrap: q.wrapW != null, wordWrapWidth: q.wrapW ?? 0 });
        const r = CanvasTextMetrics.measureText(q.text || ' ', style);
        return { ...q, w: Math.ceil(r.width), h: Math.ceil(r.height) };
      });
    }, queries),
  };
}

function runRunner(input) {
  const r = spawnSync(RUNNER, [], { input: JSON.stringify(input), encoding: 'utf8', maxBuffer: 1 << 30 });
  if (r.error) throw new Error(`cannot run ${RUNNER}: ${r.error.message} (cargo build -p meldframe-oracle-runner)`);
  if (r.status !== 0) throw new Error(`oracle-runner failed: ${r.stderr.slice(0, 500)}`);
  return JSON.parse(r.stdout);
}

const keyOf = (q) => JSON.stringify([q.text, q.fontSize, q.fontWeight, q.wrapW]);

/** Runs the runner until no query is missing from the table: each round measures in the browser what the last one asked
 *  for and could not get. `imports` are hand-written pages the runner imports (`import.mjs`). Returns the final answer, the table, and the number of runs. */
export async function solve(fixtures, mp, table = new Map(), imports = []) {
  for (let round = 1; round <= 40; round++) {
    const out = runRunner({ fixtures, imports, table: [...table.values()] });
    const seen = new Map();
    for (const f of [...out.fixtures, ...out.imports]) for (const q of f.misses) seen.set(keyOf(q), q);
    if (!seen.size) return { out, table, rounds: round };
    for (const e of await mp.measure([...seen.values()])) table.set(keyOf(e), e);
  }
  throw new Error('the text table did not settle in 40 runs');
}

// ---- Tailwind: the v4 compiler on the input stylesheet of the export plus the utilities its HTML uses ----
let tw;
async function tailwind() {
  tw ??= await import(pathToFileURL(path.join(NM, 'tailwindcss/dist/lib.mjs')).href);
  return tw;
}
/** Builds the stylesheet a Tailwind export links: `css` is its input, the candidates are the classes of `html`. */
export async function compileTailwind(css, html) {
  const { compile } = await tailwind();
  const twDir = path.join(NM, 'tailwindcss');
  const compiler = await compile(css, {
    base: twDir,
    loadStylesheet: async (id, base) => {
      const file = id.startsWith('tailwindcss/') ? path.join(NM, id) : path.resolve(base, id);
      return { path: file, base: path.dirname(file), content: fs.readFileSync(file, 'utf8') };
    },
  });
  const cands = new Set();
  for (const m of html.matchAll(/class="([^"]*)"/g)) for (const c of m[1].replace(/&quot;/g, '"').replace(/&amp;/g, '&').split(/\s+/)) if (c) cands.add(c);
  return compiler.build([...cands]);
}

// ---- pages: the export as it is delivered, a directory with the HTML page, its stylesheet, and the font and assets from the server ----
/** Writes a page of an export and returns its URL. `form` is 'css' or 'tailwind'. */
export async function writePage(base, name, form, exp) {
  const dir = path.join(OUT, 'pages', name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'index.html'), exp.html);
  if (form === 'tailwind') fs.writeFileSync(path.join(dir, 'meldframe.css'), await compileTailwind(exp.css, exp.html));
  return `${base}/out/pages/${name}/index.html`;
}

/** Waits until every weight of Inter is loaded in the page; `fonts.ready` alone can resolve before the font was asked for. */
export async function fontsReady(page) {
  await page.evaluate(async () => {
    await Promise.all([100, 200, 300, 400, 500, 600, 700, 800, 900].map((w) => document.fonts.load(`${w} 16px 'Inter Variable'`)));
    await document.fonts.ready;
  });
  return page.evaluate(() => document.fonts.check("16px 'Inter Variable'"));
}

export const domRects = (page) =>
  page.evaluate(() => {
    const out = {};
    for (const el of document.querySelectorAll('[data-node]')) {
      const r = el.getBoundingClientRect();
      out[el.getAttribute('data-node')] = { x: r.x + scrollX, y: r.y + scrollY, w: r.width, h: r.height };
    }
    return out;
  });

/** Largest absolute difference of x, y, w, h per node; `missing` when the page has no such node. */
export function diffRects(expected, actual) {
  return Object.entries(expected).map(([id, e]) => {
    const a = actual[id];
    if (!a) return { id, missing: true, max: Infinity };
    const d = { x: a.x - e.x, y: a.y - e.y, w: a.w - e.w, h: a.h - e.h };
    return { id, max: Math.max(...Object.values(d).map(Math.abs)), d };
  });
}

/** How many pixels differ between two decoded screenshots of the same size. */
export function diffPixels(a, b) {
  if (a.width !== b.width || a.height !== b.height) return Infinity;
  let n = 0;
  for (let i = 0; i < a.data.length; i += 4) if (a.data[i] !== b.data[i] || a.data[i + 1] !== b.data[i + 1] || a.data[i + 2] !== b.data[i + 2]) n++;
  return n;
}

const decoders = new WeakMap();
/** A PNG as a pixel reader: `(x, y) => [r, g, b]`, decoded by a page of `browser` (no image library); `.data` is RGBA. */
export async function decodePng(browser, png) {
  if (!decoders.has(browser)) decoders.set(browser, browser.newContext().then((c) => c.newPage()));
  const page = await decoders.get(browser);
  const img = await page.evaluate(async (b64) => {
    const i = new Image(); i.src = `data:image/png;base64,${b64}`; await i.decode();
    const c = document.createElement('canvas'); c.width = i.width; c.height = i.height;
    const g = c.getContext('2d'); g.drawImage(i, 0, 0);
    const d = g.getImageData(0, 0, i.width, i.height).data;
    let s = '';
    for (let k = 0; k < d.length; k += 8192) s += String.fromCharCode(...d.subarray(k, k + 8192));
    return { width: i.width, height: i.height, data: btoa(s) };
  }, png.toString('base64'));
  const data = Buffer.from(img.data, 'base64');
  const px = (x, y) => { const o = (y * img.width + x) * 4; return [data[o], data[o + 1], data[o + 2]]; };
  return Object.assign(px, { width: img.width, height: img.height, data });
}

export const SETTINGS = [['1280@1', { width: 1280, height: 900 }, 1], ['700@1', { width: 700, height: 500 }, 1], ['1280@2', { width: 1280, height: 900 }, 2]];
