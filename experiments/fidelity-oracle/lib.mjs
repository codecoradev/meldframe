// Shared helpers: static server, Chromium, measurement page (Pixi CanvasTextMetrics + Inter, same style as the app).
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

export const HERE = path.dirname(fileURLToPath(import.meta.url));
export const OUT = path.join(HERE, 'out');
export const RESULTS = path.join(HERE, 'results');
export const SPIKE = process.env.SPIKE_DIR ?? '/home/user/wonder-proto';
export const FONT = process.env.FONT_PATH ?? path.join(SPIKE, 'tools/capture/assets/inter.woff2');
export const BIN = path.join(process.env.CARGO_TARGET_DIR ?? '/home/user/meldframe-wt/target-b1', 'release/spike-export');
const VENDOR = {
  '/vendor/pixi.min.js': path.join(SPIKE, 'node_modules/pixi.js/dist/pixi.min.js'),
  '/vendor/layout.js': path.join(SPIKE, 'src/lib/layout.js'),
  '/font/inter.woff2': FONT,
};
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2', '.png': 'image/png' };

export function serve() {
  fs.mkdirSync(OUT, { recursive: true });
  const server = http.createServer((req, res) => {
    const u = decodeURIComponent((req.url ?? '/').split('?')[0]);
    let p = VENDOR[u] ?? (u.startsWith('/out/') ? path.join(OUT, u.slice(5)) : null);
    if (!p || !p.startsWith('/') || !fs.existsSync(p) || fs.statSync(p).isDirectory()) return res.writeHead(404).end('nf');
    res.writeHead(200, { 'content-type': MIME[path.extname(p)] ?? 'application/octet-stream' });
    fs.createReadStream(p).pipe(res);
  });
  return new Promise((r) => server.listen(0, '127.0.0.1', () => r({ base: `http://127.0.0.1:${server.address().port}`, close: () => server.close() })));
}

export async function launch() {
  const req = createRequire('/opt/node-tools/node_modules/');
  return req('playwright').chromium.launch(); // no options, as specified
}

export const MEASURE_HTML = `<!doctype html><meta charset=utf-8><style>
@font-face{font-family:'Inter Variable';src:url('/font/inter.woff2') format('woff2');font-weight:100 900;font-display:block}
body{margin:0;font-family:'Inter Variable'}</style><span>Inter</span>
<script src="/vendor/pixi.min.js"></script>`;

/** Opens the measurement page. Returns {layout(nodes) -> {rects, table}} running the TS oracle (src/lib/layout.js) with a
 *  weight-aware Pixi measurer that records every query (the table the Rust core consumes). */
export async function openMeasurePage(browser, base) {
  fs.writeFileSync(path.join(OUT, 'measure.html'), MEASURE_HTML);
  const page = await (await browser.newContext()).newPage();
  await page.goto(`${base}/out/measure.html`);
  await page.evaluate(async () => {
    await document.fonts.load('400 16px "Inter Variable"');
    await document.fonts.load('700 16px "Inter Variable"');
    await document.fonts.load('900 16px "Inter Variable"');
    await document.fonts.ready;
    window.__layoutMod = await import('/vendor/layout.js');
  });
  const fontOk = await page.evaluate(() => document.fonts.check('16px "Inter Variable"'));
  if (!fontOk) throw new Error('Inter did not load in measurement page');
  return {
    page,
    async lineHeight(fontSize, fontWeight = 400) {
      return page.evaluate(([fs, w]) => { const { CanvasTextMetrics, TextStyle } = window.PIXI; return Math.ceil(CanvasTextMetrics.measureText('Hg', new TextStyle({ fontFamily: 'Inter Variable, system-ui, sans-serif', fontSize: fs, fontWeight: String(w) })).height); }, [fontSize, fontWeight]);
    },
    async layout(nodes) {
      return page.evaluate((nodes) => {
        const { CanvasTextMetrics, TextStyle } = window.PIXI;
        const FONT = 'Inter Variable, system-ui, sans-serif';
        const table = [];
        const seen = new Map();
        const measure = (n, wrapW) => {
          const k = `${n.text}|${n.fontSize}|${wrapW ?? null}`;
          const wkey = n.fontWeight ?? 400;
          const style = new TextStyle({ fontFamily: FONT, fontSize: n.fontSize, fontWeight: String(wkey), wordWrap: wrapW != null, wordWrapWidth: wrapW ?? 0 });
          const r = CanvasTextMetrics.measureText(n.text || ' ', style);
          const m = { w: Math.ceil(r.width), h: Math.ceil(r.height) };
          if (!seen.has(k)) { seen.set(k, wkey); table.push({ text: n.text, fontSize: n.fontSize, wrapW: wrapW ?? null, w: m.w, h: m.h }); }
          else if (seen.get(k) !== wkey) table.push({ conflict: true });
          return m;
        };
        const rects = window.__layoutMod.computeRects(nodes, measure);
        for (const n of nodes) if (n.type === 'text') measure(n, null); // line-height query used by the export
        const out = {};
        for (const [id, r] of rects) out[id] = { x: r.x, y: r.y, w: r.w, h: r.h };
        return { rects: out, table: table.filter((e) => !e.conflict), conflicts: table.filter((e) => e.conflict).length };
      }, nodes);
    },
  };
}

export function cli(nodesPath, tablePath, format, extra = []) {
  const r = spawnSync(BIN, [nodesPath, '--measure', tablePath, '--format', format, ...extra], { encoding: 'utf8', maxBuffer: 256 << 20 });
  return { status: r.status, out: r.stdout, err: r.stderr, warnings: r.stderr.split('\n').filter((l) => l.startsWith('PERINGATAN')) };
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

export function diffRects(expected, actual) {
  const rows = [];
  for (const [id, e] of Object.entries(expected)) {
    const a = actual[id];
    if (!a) { rows.push({ id, missing: true }); continue; }
    const d = { x: a.x - e.x, y: a.y - e.y, w: a.w - e.w, h: a.h - e.h };
    rows.push({ id, d, max: Math.max(...Object.values(d).map(Math.abs)), exp: e, act: a });
  }
  return rows;
}
