// The canvas and the exported page draw the same bound values (issue 254, docs/tokens-and-code.md). token-loop-assert.mjs reads text: it checks that
// the exported CSS holds the values get_variables reads back. This script reads pixels: it imports a copy of the token CSS of that script (with a radius and an opacity added), binds a frame to the variables
// (fill, radius and opacity), takes a `capture` of the frame from an editor open in Chromium (what the canvas draws), loads the exported page
// (`export_frame`) in the same browser, and asserts that at a few points the two pixels are equal within 4/255 per channel and are the colour the
// imported value gives. Then it changes the variables with `setVariableValue` (the loop's two-way step) and asserts again: the canvas and the export both follow.
//   node packages/web-e2e/scripts/token-loop-canvas-e2e.mjs
// By hand, from the repository root, with Playwright on the global module path (as the other scripts). Needs the server binary
// (`cargo build -p meldframe-server`; `MELDFRAME_SERVER_BIN` overrides `$CARGO_TARGET_DIR/debug/meldframe-server`), the WASM bridge built from this
// revision (`tools/build-wasm.sh`) and `npm ci` in apps/web. The script starts a fresh in-memory server and one Vite on a file whose name is new for each
// run, and stops both at the end, on an error and on SIGINT and SIGTERM. The children get NO `DATABASE_URL`, no other variable with DATABASE or a PG
// prefix in its name and no `MELDFRAME_*` variable of the caller's environment (they are rebuilt from the ones this script sets): the server binary connects
// to, and migrates, a Postgres when `MELDFRAME_LISTEN` and `DATABASE_URL` are both set, and this script must never write into a development database.
// Prints one line per check and exits 1 when a check fails. Not checked: the dark mode on the canvas (packages/web-panels/scripts/variables-e2e.mjs
// switches the mode), and text (the export measures text with a stub, see docs/tokens-and-code.md).
//
// WHAT EACH PROBE PROVES. After the import the probes agree with the CSS text, but a literal would give the same pixels; the binding shows when the values
// change: the page fill follows `surface`, the card fill `brand`, the card's blend `opacity-muted`, the card's and the bar's corner pixel at (44, 44) and
// (44, 164) `radius-card` (painted at radius 12, cut at 20), the bar fill `accent`, an alias of `brand`. The outer corners (41, 41) and (41, 161) are cut at
// both radii: they only show that a radius is drawn.
//
// THE RASTERISER. The browser runs on SwiftShader, a software rasteriser (`--use-gl=swiftshader`, as in shadow-perf.mjs; `MELDFRAME_GL_ARGS="<flags>"`
// replaces the flags, an empty value takes the machine's own choice), and the output names the WebGL renderer it got. The 0/255 and the 4/255 below were
// measured on that rasteriser; a GPU may round differently, which is what the tolerance is for.
//
// THE TOLERANCE is 4/255 per channel. The canvas (Pixi on WebGL) and the exported page (CSS) are two renderers; they round a blend with `opacity`
// differently, by at most 1/255 where shadow-e2e.mjs measures it. docs/fidelity-contract.md allows 40/255 near edges for anti-aliasing; 4 is the margin for
// the rounding of the blend and no more, so a wrong variable, radius or opacity (a difference of tens of levels) fails. A probe near an edge: an
// anti-aliased edge spreads over about 1 px, so a pixel whose centre is 1 px or more from the arc is wholly inside or outside it. The closest probe, (44, 44),
// is 1.4 px inside the arc of radius 12 (1.9 px outside that of radius 20); the script computes each corner probe's expected colour from the radius and the
// pixel centre, and stops with an error for a probe closer than 1 px to an arc. The canvas is polled until it shows the expected values (the editor
// receives an update over the socket, a moment after the command): the poll ends after 20 s, and the check that follows then fails on what the last
// capture held, or the error says what the last answer to `capture` was.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright');
const root = fileURLToPath(new URL('../../../', import.meta.url));
const GL_ARGS = process.env.MELDFRAME_GL_ARGS === undefined ? ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] : process.env.MELDFRAME_GL_ARGS.split(' ').filter(Boolean);
const TOL = 4;
const run = Math.random().toString(36).slice(2, 8);
const file = `tokenloopcanvas_${run}`;
const token = randomBytes(16).toString('hex');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const freePort = () => new Promise((res, rej) => { const s = createServer().listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => res(port)); }).on('error', rej); });

// The environment of a child: the caller's without DATABASE_URL (or any DATABASE or PG variable) and without MELDFRAME_* variables, plus `env`.
const childEnv = (env) => ({ ...Object.fromEntries(Object.entries(process.env).filter(([k]) => !/DATABASE|^PG|^MELDFRAME_/i.test(k))), ...env });
const children = [];
let browser, stopping = false;
const killChildren = (sig) => { for (const p of children) { try { process.kill(-p.pid, sig); } catch { /* gone already */ } } };
const stopAll = async () => {
  stopping = true;
  try { await browser?.close(); } catch { /* closed already */ }
  killChildren('SIGTERM');
  await sleep(300);
  killChildren('SIGKILL');
};
const fatal = async (err) => { console.error(err instanceof Error ? err.stack ?? err.message : err); await stopAll(); process.exit(1); };
for (const sig of ['SIGINT', 'SIGTERM']) process.on(sig, async () => { console.error(`${sig}: stopping`); await stopAll(); process.exit(130); });
process.on('uncaughtException', fatal);
process.on('unhandledRejection', fatal);
process.on('exit', () => killChildren('SIGKILL'));

/** Starts a child in a group of its own (so the whole tree can be stopped). `dies` rejects, with what the child printed, when it ends or fails to start. */
const start = (what, cmd, args, env, cwd) => {
  const p = spawn(cmd, args, { cwd, env: childEnv(env), stdio: ['ignore', 'ignore', 'pipe'], detached: true });
  let err = '';
  p.stderr.on('data', (d) => { err = (err + d).slice(-4000); });
  p.dies = new Promise((_, rej) => {
    p.on('error', (e) => rej(new Error(`${what}: could not start ${cmd}: ${e.message}`)));
    p.on('exit', (code, sig) => { if (!stopping) rej(new Error(`${what} ended (${sig ?? `exit ${code}`}) before the run was over${err ? `; it printed:\n${err}` : ''}`)); });
  });
  p.dies.catch(fatal);
  children.push(p);
  return p;
};
const waitFor = async (url, what, child) => {
  let up = false;
  const poll = (async () => {
    for (let i = 0; i < 120 && !up; i++) { try { if ((await fetch(url)).ok) { up = true; return; } } catch { /* not up yet */ } await sleep(500); }
    throw new Error(`${what} did not come up at ${url}`);
  })();
  poll.catch(() => {});
  await Promise.race([poll, child.dies]);
};

// ---- the agent's side: MCP tool calls on /mcp with the token ----
let server, n = 0;
const short = (t) => String(t).replace(/\s+/g, ' ').slice(0, 300);
const rpc = async (name, args) => {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++n, method: 'tools/call', params: { name, arguments: args } }),
  });
  const raw = await res.text();
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}: ${short(raw)}`);
  const sse = (res.headers.get('content-type') ?? '').includes('text/event-stream');
  let j;
  try { j = JSON.parse(sse ? raw.split('\n').filter((l) => l.startsWith('data:')).map((l) => l.slice(5).trim()).join('') : raw); } catch { throw new Error(`${name}: the answer is not JSON: ${short(raw)}`); }
  if (j.error) throw new Error(`${name}: JSON-RPC ${j.error.code}: ${j.error.message}`);
  if (!j.result) throw new Error(`${name}: no result in the answer: ${short(raw)}`);
  return j.result;
};
const text = (r) => r.content?.find((c) => c.type === 'text')?.text;
/** A tool that answers JSON: an `isError` answer, an `error` field, no body or a body that is not JSON all throw, with what the server said. */
const tool = async (name, args) => {
  const r = await rpc(name, { file, ...args });
  if (r.isError) throw new Error(`${name} was refused: ${short(text(r) ?? JSON.stringify(r.structuredContent ?? r))}`);
  let body = r.structuredContent;
  if (!body) {
    const t = text(r);
    if (t === undefined) throw new Error(`${name}: the answer has no body: ${short(JSON.stringify(r))}`);
    try { body = JSON.parse(t); } catch { throw new Error(`${name}: the answer is not JSON: ${short(t)}`); }
  }
  assert.ok(!body.error && body.ok !== false, `${name} failed: ${short(JSON.stringify(body))}`);
  return body;
};

// ---- pixels: a PNG read in a page of the browser ----
const pixels = (page, b64, points) => page.evaluate(async ({ b64, points }) => {
  const img = new Image();
  img.src = `data:image/png;base64,${b64}`;
  await img.decode();
  const c = document.createElement('canvas');
  c.width = img.width; c.height = img.height;
  const g = c.getContext('2d');
  g.drawImage(img, 0, 0);
  return { w: img.width, h: img.height, rows: points.map(([, x, y]) => Array.from(g.getImageData(x, y, 1, 1).data.slice(0, 3))) };
}, { b64, points });
const hex = (s) => {
  if (!/^#[0-9a-f]{6}$/i.test(s)) throw new Error(`"${s}" is not a #rrggbb colour: this script reads nothing else`);
  return [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16));
};
const mix = (top, under, a) => top.map((v, k) => Math.round(a * v + (1 - a) * under[k]));
const near = (a, b) => a.every((v, k) => Math.abs(v - b[k]) <= TOL);
const rgb = (c) => `rgb(${c.join(',')})`;

let failed = 0;
const check = (ok, what, detail = '') => { console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}${detail ? `: ${detail}` : ''}`); if (!ok) failed++; };

// The token CSS of token-loop-assert.mjs (repeated here: that script does not export it), with two number variables added, which it has not:
// a radius and an opacity, so that a bound radius and a bound opacity are drawn too.
const css = `:root {
  --brand: #1D4ED8;
  --surface: #ffffff;
  --ink: rgb(17 24 39);
  --accent: var(--brand);
  --radius-card: 12px;
  --opacity-muted: 0.6;
}
.dark {
  --surface: #0b1220;
}
`;
// What the CSS says, read from the text and not from the server: the colours the canvas and the page have to draw.
const sheet = Object.fromEntries([...css.split('.dark')[0].matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));

async function main() {
  const serverBin = process.env.MELDFRAME_SERVER_BIN ?? `${process.env.CARGO_TARGET_DIR ?? `${root}target`}/debug/meldframe-server`;
  const sp = await freePort(), wp = await freePort();
  server = `http://127.0.0.1:${sp}`;
  const web = `http://127.0.0.1:${wp}`;
  const serverChild = start('the server', serverBin, [], { MELDFRAME_DEV_TOKENS: `${token}:ws_dev:agent:dev`, MELDFRAME_LISTEN: `127.0.0.1:${sp}` }, root);
  await waitFor(`${server}/v1/health`, 'the server', serverChild);
  const vite = start('vite', `${root}apps/web/node_modules/.bin/vite`, ['--port', String(wp), '--strictPort', '--host', '127.0.0.1'], { MELDFRAME_SERVER: server, MELDFRAME_WEB_TOKEN: token, MELDFRAME_WEB_FILE: file }, `${root}apps/web`);
  await waitFor(web, 'vite', vite);
  console.log(`token loop canvas ${new Date().toISOString()}  file ${file}  server ${server}`);

  // 1. CSS to canvas: import, read back, bind a page (fill), a card (fill, radius, opacity) and a bar (fill through the alias, radius), one command per field.
  await tool('import_tokens', { css, dryRun: false, idempotencyKey: `import-${run}` });
  const vars = await tool('get_variables', {});
  const id = Object.fromEntries(vars.variables.map((v) => [v.name, v.id]));
  const modeOf = (v) => vars.collections.find((c) => c.id === vars.variables.find((x) => x.name === v).collection).modes[0].id;
  check(['surface', 'brand', 'accent', 'radius-card', 'opacity-muted'].every((k) => id[k]), 'import_tokens made the variables', Object.keys(id).join(', '));
  const nodes = [
    { id: 'page', type: 'frame', name: 'page', x: 0, y: 0, w: 400, h: 240, fill: { var: id.surface } },
    { id: 'card', type: 'frame', name: 'card', parentId: 'page', x: 40, y: 40, w: 320, h: 100, fill: { var: id.brand }, radius: { var: id['radius-card'] }, opacity: { var: id['opacity-muted'] } },
    { id: 'bar', type: 'rect', name: 'bar', parentId: 'page', x: 40, y: 160, w: 320, h: 44, fill: { var: id.accent }, radius: { var: id['radius-card'] } },
  ];
  await tool('apply_commands', { idempotencyKey: `build-${run}`, commands: nodes.map((node) => ({ type: 'addNode', node })) });

  // The canvas: an editor open on the file; `capture` is answered by it.
  browser = await chromium.launch({ args: GL_ARGS });
  const edit = await browser.newPage({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: 1 });
  edit.on('pageerror', (e) => { failed++; console.log('[pageerror]', e.message); });
  await edit.goto(web);
  await edit.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 60_000 });
  await edit.waitForTimeout(2500);
  const gl = await edit.evaluate(() => {
    const c = document.querySelector('canvas'), g = c?.getContext('webgl2') ?? c?.getContext('webgl'), e = g?.getExtension('WEBGL_debug_renderer_info');
    return g && e ? String(g.getParameter(e.UNMASKED_RENDERER_WEBGL)) : 'unknown';
  });
  console.log(`browser   Chromium ${browser.version()}, WebGL renderer "${gl}", GL flags: ${GL_ARGS.join(' ') || '(none)'}`);
  if (/swiftshader|llvmpipe|software/i.test(gl)) console.log('NOTE      a software rasteriser: the 0/255 and the 4/255 hold for it, not measured on a GPU');
  const ref = await browser.newPage({ viewport: { width: 400, height: 240 }, deviceScaleFactor: 1 });

  // [what, kind, role, x, y of the pixel in the 400 x 240 frame]; a `corner` is in the top left corner region of its box. The card is the box (40, 40, 320 x 100), the bar (40, 160, 320 x 44), both with the bound radius.
  const probes = [
    ['page fill', 'page', 'fill', 10, 10], ['card fill at its opacity', 'card', 'fill', 200, 90], ['card outer corner (41, 41), cut at both radii', 'card', 'corner', 41, 41],
    ['card corner (44, 44), painted at radius 12 and cut at 20', 'card', 'corner', 44, 44], ['bar fill (alias of brand)', 'bar', 'fill', 200, 182],
    ['bar outer corner (41, 161), cut at both radii', 'bar', 'corner', 41, 161], ['bar corner (44, 164), painted at radius 12 and cut at 20', 'bar', 'corner', 44, 164],
  ].map(([what, kind, role, x, y]) => [what, x, y, kind, role]);
  const BOX = { card: { x: 40, y: 40, h: 100 }, bar: { x: 40, y: 160, h: 44 } };
  /** Is the pixel (px, py) of the top left corner region of `box` painted, for a corner radius r? From the distance of its centre to the arc's centre (r, r). */
  const painted = (px, py, box, r) => {
    if (!Number.isFinite(r) || r < 1 || r > box.h / 2) throw new Error(`the radius ${r} is outside what the probes support (1 to ${box.h / 2})`);
    const cx = px + 0.5 - box.x, cy = py + 0.5 - box.y;
    if (cx >= r || cy >= r) return true;
    const d = Math.hypot(r - cx, r - cy);
    if (Math.abs(d - r) < 1) throw new Error(`the probe (${px}, ${py}) is ${Math.abs(d - r).toFixed(2)} px from the arc of radius ${r}: too close to an edge`);
    return d < r;
  };
  const want = (v) => {
    if (!(v.opacity >= 0 && v.opacity <= 1)) throw new Error(`the opacity ${v.opacity} is outside 0 to 1`);
    const card = mix(v.brand, v.surface, v.opacity);
    const fill = { card, bar: v.brand };
    return probes.map(([, x, y, kind, role]) => {
      if (kind === 'page') return v.surface;
      return role === 'fill' || painted(x, y, BOX[kind], v.radius) ? fill[kind] : v.surface;
    });
  };
  const canvasOf = async (expect) => {
    let last, note = 'none';
    for (let i = 0; i < 40; i++) {
      const r = await rpc('capture', { file, nodeId: 'page' });
      const part = r.content?.find((c) => c.type === 'image');
      if (part) {
        last = { ...(await pixels(edit, part.data, probes)) };
        if (last.rows.every((px, k) => near(px, expect[k]))) return last;
      } else {
        note = `${r.isError ? 'error' : 'no image'}: ${short(text(r) ?? JSON.stringify(r))}`;
      }
      await sleep(500);
    }
    if (last) return last;
    throw new Error(`the editor answered no capture in 20 s; the last answer was ${note}`);
  };
  const pageOf = async (html) => { await ref.setContent(html); return pixels(ref, (await ref.screenshot()).toString('base64'), probes); };
  let worst = 0;
  const compare = async (label, v) => {
    const expect = want(v);
    const canvas = await canvasOf(expect);
    const ex = await tool('export_frame', { frame: 'page' });
    const page = await pageOf(ex.html);
    check(canvas.w === 400 && canvas.h === 240, `${label}: the capture is 400 x 240`, `${canvas.w} x ${canvas.h}`);
    probes.forEach(([what], k) => {
      const d = Math.max(...canvas.rows[k].map((c, i) => Math.abs(c - page.rows[k][i])));
      worst = Math.max(worst, d);
      check(near(canvas.rows[k], page.rows[k]) && near(page.rows[k], expect[k]), `${label}: ${what}`, `canvas ${rgb(canvas.rows[k])}, export ${rgb(page.rows[k])}, the values give ${rgb(expect[k])} (canvas vs export ${d}/255)`);
    });
    return ex;
  };

  // 2. Canvas to code: the pixels are the canvas's and the CSS text's, and the export writes the card's fill, radius and opacity as var(...).
  const px = /^(\d+)px$/.exec(sheet['radius-card'] ?? '');
  if (!px) throw new Error(`--radius-card is "${sheet['radius-card']}": this script reads a whole number of px`);
  const v1 = { surface: hex(sheet.surface), brand: hex(sheet.brand), opacity: Number(sheet['opacity-muted']), radius: Number(px[1]) };
  const ex1 = await compare('imported', v1);
  const decls = (body) => Object.fromEntries(body.split(';').filter(Boolean).map((d) => [d.slice(0, d.indexOf(':')), d.slice(d.indexOf(':') + 1)]));
  const rule = (html, sel) => decls(new RegExp(`${sel}\\{([^}]*)\\}`).exec(html)?.[1] ?? '');
  const card = rule(ex1.html, '\\[data-node="card"\\]');
  check(card['background-color'] === 'var(--root-brand)' && card['border-radius'] === 'calc(var(--root-radius-card) * 1px)' && card.opacity === 'var(--root-opacity-muted)', 'the export writes the card as var(...) for fill, radius and opacity', JSON.stringify(card));

  // 3. Two-way: the values change on the file (what an agent does when the stylesheet changed); the canvas and the export both follow.
  const next = { surface: '#fef3c7', brand: '#7c3aed', radius: 20, opacity: 0.4 };
  await tool('apply_commands', { idempotencyKey: `change-${run}`, commands: [
    { type: 'setVariableValue', id: id.surface, mode: modeOf('surface'), value: next.surface },
    { type: 'setVariableValue', id: id.brand, mode: modeOf('brand'), value: next.brand },
    { type: 'setVariableValue', id: id['radius-card'], mode: modeOf('radius-card'), value: next.radius },
    { type: 'setVariableValue', id: id['opacity-muted'], mode: modeOf('opacity-muted'), value: next.opacity },
  ] });
  const ex2 = await compare('after setVariableValue', { surface: hex(next.surface), brand: hex(next.brand), opacity: next.opacity, radius: next.radius });
  const root2 = rule(ex2.html, ':root');
  const wantRoot = { '--root-surface': next.surface, '--root-brand': next.brand, '--root-radius-card': String(next.radius), '--root-opacity-muted': String(next.opacity) };
  const wrong = Object.entries(wantRoot).filter(([k, v]) => root2[k] !== v).map(([k, v]) => `${k} is ${root2[k]}, want ${v}`);
  check(wrong.length === 0, 'the second export carries the new values in its :root rule', wrong.join('; ') || JSON.stringify(root2));
  console.log(`largest difference between a canvas pixel and an exported pixel: ${worst}/255 per channel (tolerance ${TOL})`);
}

try {
  await main();
  console.log(failed ? `${failed} check(s) FAILED` : 'TOKEN LOOP CANVAS OK');
  if (failed) process.exitCode = 1;
} finally {
  await stopAll();
}
