// The canvas and the exported page draw the same bound values (issue 254, docs/tokens-and-code.md). token-loop-assert.mjs reads text: it checks that
// the exported CSS holds the values get_variables reads back. This script reads pixels: it imports the same token CSS, binds a frame to the variables
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
// The tolerance is 4/255 per channel. The canvas (Pixi on WebGL) and the exported page (CSS) are two renderers; they round a blend with `opacity`
// differently, by at most 1/255 where shadow-e2e.mjs measures it, and the probes here sit 2 px or more from an edge. docs/fidelity-contract.md allows
// 40/255 near edges for anti-aliasing; 4 is the margin for the rounding of the blend and no more, so a wrong variable, radius or opacity (a difference of
// tens of levels) fails. The canvas is polled until it shows the expected values (the editor receives an update over the socket, a moment after the
// command): the poll ends after 20 s, and the check that follows then fails on what the last capture held.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const { chromium } = createRequire(import.meta.url)('playwright');
const root = fileURLToPath(new URL('../../../', import.meta.url));
const GL_ARGS = ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'];
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
const rpc = async (name, args) => {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++n, method: 'tools/call', params: { name, arguments: args } }),
  });
  const j = await res.json();
  if (!j.result) throw new Error(`${name}: ${JSON.stringify(j)}`);
  return j.result;
};
const tool = async (name, args) => {
  const r = await rpc(name, { file, ...args });
  const body = r.structuredContent ?? JSON.parse(r.content?.[0]?.text ?? '{}');
  assert.ok(!r.isError && !body.error && body.ok !== false, `${name} failed: ${JSON.stringify(body).slice(0, 400)}`);
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
const hex = (s) => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16));
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

  // 1. CSS to canvas: import, read back, bind a page, a card and a bar (fill, radius and opacity bound, one field each).
  await tool('import_tokens', { css, dryRun: false, idempotencyKey: `import-${run}` });
  const vars = await tool('get_variables', {});
  const id = Object.fromEntries(vars.variables.map((v) => [v.name, v.id]));
  const modeOf = (v) => vars.collections.find((c) => c.id === vars.variables.find((x) => x.name === v).collection).modes[0].id;
  check(['surface', 'brand', 'accent', 'radius-card', 'opacity-muted'].every((k) => id[k]), 'import_tokens made the variables', Object.keys(id).join(', '));
  const nodes = [
    { id: 'page', type: 'frame', name: 'page', x: 0, y: 0, w: 400, h: 240, fill: { var: id.surface } },
    { id: 'card', type: 'frame', name: 'card', parentId: 'page', x: 40, y: 40, w: 320, h: 100, fill: { var: id.brand }, radius: { var: id['radius-card'] }, opacity: { var: id['opacity-muted'] } },
    { id: 'bar', type: 'rect', name: 'bar', parentId: 'page', x: 40, y: 160, w: 320, h: 24, fill: { var: id.accent }, radius: { var: id['radius-card'] } },
  ];
  await tool('apply_commands', { idempotencyKey: `build-${run}`, commands: nodes.map((node) => ({ type: 'addNode', node })) });

  // The canvas: an editor open on the file; `capture` is answered by it.
  browser = await chromium.launch({ args: GL_ARGS });
  const edit = await browser.newPage({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: 1 });
  edit.on('pageerror', (e) => { failed++; console.log('[pageerror]', e.message); });
  await edit.goto(web);
  await edit.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 60_000 });
  await edit.waitForTimeout(2500);
  const ref = await browser.newPage({ viewport: { width: 400, height: 240 }, deviceScaleFactor: 1 });

  // [what, x, y of the pixel in the 400 x 240 frame]. (41, 41) is 2.8 px from the arc of the card's corner of radius 12 and outside it; (44, 44) is inside it
  // (distance 10.6 from the centre of the arc) and outside the arc of radius 20; the bar's corner is outside its arc.
  const probes = [
    ['page fill (bound colour)', 10, 10], ['card fill at the bound opacity', 200, 90], ['card corner cut by the bound radius', 41, 41],
    ['card pixel (44, 44): painted at radius 12, cut at 20', 44, 44], ['bar fill (an alias of brand)', 200, 172], ['bar corner cut by the bound radius', 41, 161],
  ];
  const want = (v) => {
    const card = mix(v.brand, v.surface, v.opacity);
    return [v.surface, card, v.surface, v.radius >= 20 ? v.surface : card, v.brand, v.surface];
  };
  const canvasOf = async (expect) => {
    let last;
    for (let i = 0; i < 40; i++) {
      const part = (await rpc('capture', { file, nodeId: 'page' })).content?.find((c) => c.type === 'image');
      if (part) {
        last = { ...(await pixels(edit, part.data, probes)) };
        if (last.rows.every((px, k) => near(px, expect[k]))) return last;
      }
      await sleep(500);
    }
    if (last) return last;
    throw new Error('the editor answered no capture');
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

  // 2. Canvas to code: the export writes the bound fields as var(...), and its pixels are the canvas's.
  const v1 = { surface: hex(sheet.surface), brand: hex(sheet.brand.toLowerCase()), opacity: Number(sheet['opacity-muted']), radius: parseInt(sheet['radius-card'], 10) };
  const ex1 = await compare('imported', v1);
  const rule = (html, node) => new RegExp(`\\[data-node="${node}"\\]\\{([^}]*)\\}`).exec(html)?.[1] ?? '';
  const card = rule(ex1.html, 'card');
  check(/background-color:var\(--root-brand\)/.test(card) && /border-radius:calc\(var\(--root-radius-card\) \* 1px\)/.test(card) && /opacity:var\(--root-opacity-muted\)/.test(card), 'the export writes the card as var(...) for fill, radius and opacity', card);

  // 3. Two-way: the values change on the file (what an agent does when the stylesheet changed); the canvas and the export both follow.
  const next = { brand: '#7c3aed', radius: 20, opacity: 0.4 };
  await tool('apply_commands', { idempotencyKey: `change-${run}`, commands: [
    { type: 'setVariableValue', id: id.brand, mode: modeOf('brand'), value: next.brand },
    { type: 'setVariableValue', id: id['radius-card'], mode: modeOf('radius-card'), value: next.radius },
    { type: 'setVariableValue', id: id['opacity-muted'], mode: modeOf('opacity-muted'), value: next.opacity },
  ] });
  const ex2 = await compare('after setVariableValue', { surface: v1.surface, brand: hex(next.brand), opacity: next.opacity, radius: next.radius });
  check(ex2.html.includes('--root-brand:#7c3aed') && ex2.html.includes('--root-radius-card:20') && ex2.html.includes('--root-opacity-muted:0.4'), 'the second export carries the new values', (/:root\{[^}]*\}/.exec(ex2.html) ?? [''])[0]);
  console.log(`largest difference between a canvas pixel and an exported pixel: ${worst}/255 per channel (tolerance ${TOL})`);
}

try {
  await main();
  console.log(failed ? `${failed} check(s) FAILED` : 'TOKEN LOOP CANVAS OK');
  if (failed) process.exitCode = 1;
} finally {
  await stopAll();
}
