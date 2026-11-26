// Browser check of the Activity panel (E5.1 to E5.3, ADR-0021 slice 15) against a real server:
//   node packages/web-panels/scripts/activity-e2e.mjs [web-url] [server-url]
// The person is the browser editor (the token in .dev-token at the root); the agent is a second token that speaks MCP over
// HTTP (`apply_commands` with a note, as an agent does) and, optionally, a second browser. Start, with fresh file names
// (the numbers of a file start at 1, and a second run on the same file is counted from where the first stopped):
//   MELDFRAME_DEV_TOKENS="<.dev-token>:ws_dev:person:ada,<token 2>:ws_dev:agent:claude" MELDFRAME_REVIEWERS=person:ada just dev-server
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_WEB_ACTOR=person:ada just web                  (the person's page, port 5173)
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_WEB_ACTOR=agent:claude MELDFRAME_WEB_TOKEN=<token 2> npx vite --port 5174   (in apps/web)
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_DEMO_TOKEN2=<token 2> MELDFRAME_AGENT_WEB=http://localhost:5174 \
//     node packages/web-panels/scripts/activity-e2e.mjs
// `just dev-server` sources .env after the environment is set: a .env that sets MELDFRAME_DEV_TOKENS or MELDFRAME_REVIEWERS
// overrides the line above, so leave them out of it. MELDFRAME_WEB_FILE is required and the file must have no batches yet. The
// web and server URLs must be loopback (both tokens are sent there).
// The listed reviewer is the person's actor only (the agent must not be able to review its own work). MELDFRAME_AGENT_WEB is
// optional: without it the check of the agent's own page is reported as skipped. MELDFRAME_E2E_SHOTS=<dir> saves screenshots
// (and one of the page when a check fails); MELDFRAME_E2E_DEBUG=1 prints more of each failure.
// The Activity tab needs the WASM bridge built (`just wasm-build`) and Playwright from the global module path. Run it against a
// development server only: it adds nodes and batches to the file and leaves them. It waits about 10 s for the touch rings to
// go out and the lane to go quiet.
// The React copy of the panel (E7.3 slice S8) is checked by the same script with MELDFRAME_UI=react: the pages open with
// /?react=activity (the agent's page too), a check asks that React drew the panel (a silent fallback to Svelte would pass every
// other check), and a last check blocks the chunks of the React copy and asks that the Svelte panel is drawn and works, so that a
// chunk that does not load never leaves a card with no panel. The block matches the paths of the Vite dev server's modules, so that
// check is for the dev server only. Without MELDFRAME_UI the Svelte copy runs.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fibre, openWithChunksBlocked, pageUrl, react, rgbCount, sleep, until } from './activity-helpers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const agentWeb = process.env.MELDFRAME_AGENT_WEB;
const root = new URL('../../../', import.meta.url).pathname;
const person = readFileSync(`${root}.dev-token`, 'utf8').trim();
const agent = process.env.MELDFRAME_DEMO_TOKEN2;
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
if (!agent) throw new Error('set MELDFRAME_DEMO_TOKEN2 to the second token (see the top of this file)');
if (!file) throw new Error('set MELDFRAME_WEB_FILE to a file name that has no batches yet (see the top of this file)');
// Both tokens go to these hosts (the person's in the proxy of the page, the agent's in the calls of this script): loopback only.
for (const u of [web, server, ...(agentWeb ? [agentWeb] : [])]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send tokens to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const run = Math.random().toString(36).slice(2, 7);
const pageErrors = [];
let rpc = 0;
let mainPage; // the person's page, for a screenshot when a check fails

/** One MCP tool call as the holder of `token`: the JSON body of the result. */
async function tool(token, name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  return JSON.parse((await res.json()).result?.content?.[0]?.text ?? '{}');
}
/** One REST call: `{status, body}`. */
async function http(token, method, path, body) {
  const res = await fetch(`${server}${path}`, { method, headers: { authorization: `Bearer ${token}`, ...(body ? { 'content-type': 'application/json' } : {}) }, body: body && JSON.stringify(body) });
  return { status: res.status, body: await res.json().catch(() => null) };
}
const list = async (token) => (await http(token, 'GET', `/v1/files/${file}/batches`)).body;
const nodes = async () => Object.fromEntries((await http(person, 'GET', `/v1/files/${file}`)).body.nodes.map((n) => [n.id, n]));
const seqNow = async () => (await http(person, 'GET', `/v1/files/${file}`)).body.seq;

const id = (s) => `${s}_${run}`;
const [hero, heading, blurb, card] = ['hero', 'heading', 'blurb', 'card'].map(id);
const text = (nid, parentId, t, y, size) => ({ type: 'addNode', node: { id: nid, type: 'text', name: t, parentId, x: 32, y, w: 400, h: size + 8, text: t, fontSize: size, fill: '#ffffff' } });
const T0 = 'Design with agents', T1 = 'Design faster';
const F0 = '#1d3b6e', F1 = '#7a2e9b', PERSON = '#16a34a';
// The note is text, whatever it holds: markup, a quotation mark and a line break must reach the page as they are.
const NOTE_A = 'Builds the hero: a frame, a heading and a blurb.';
const NOTE_B = 'Retitle and recolour the hero.\n<b>not bold</b> & <img src=x onerror=alert(1)> "quoted"';
const NOTE_C = 'Adds a card under the hero.';

const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) {
    results.push(false);
    if (shots && mainPage) await mainPage.screenshot({ path: `${shots}/fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}
const skip = (name, why) => console.log(`skip ${name}: ${why}`);

(async () => {
  const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const open = async (url) => {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
    page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
    await page.goto(url);
    await until('live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
    return page;
  };
  const shot = (page, name) => (shots ? page.screenshot({ path: `${shots}/${name}.png` }) : null);
  const page = await open(pageUrl(web));
  mainPage = page;
  await page.evaluate(() => { window.__stay = true; });
  const tab = (p = page) => p.locator('#right-tab-activity');
  const badge = async (p = page) => Number.parseInt(/Activity\s*(\d+)/.exec(await tab(p).innerText())?.[1] ?? '0', 10);
  // A lane is open by index: when the person's own lane comes first, the agent's folds. Open every folded lane.
  const unfold = async (p = page) => {
    for (let i = 0; i < 8; i++) { // one at a time, asking again: a click can move the lanes
      const b = p.locator('[data-testid=lane] > button[aria-expanded=false]').first();
      if ((await b.count()) === 0) return;
      await b.click();
    }
  };
  const openActivity = async (p = page) => { await tab(p).click(); await p.locator('[data-testid=activity]').waitFor({ timeout: 10000 }); await unfold(p); };
  // Where the focus is: the test id of the nearest element that has one, and its text.
  const active = () => page.evaluate(() => { const e = document.activeElement; return `${e?.closest('[data-testid]')?.getAttribute('data-testid') ?? ''}|${e?.textContent?.trim() ?? ''}`; });
  const row = (n, p = page) => p.locator(`[data-testid=batch][data-n="${n}"]`);
  // The canvas runs under the panels: count only the strip between them (the agent colours also dot the lane and the layers).
  const canvasBox = { x: 300, y: 60, width: 740, height: 740 };
  const rings = () => rgbCount(page, canvasBox, ['#FB923C', '#2DD4BF', '#C084FC', '#C2410C', '#0E7490', '#7E22CE']);
  const baseRings = await rings();

  // First of all, as the header says: a silent fallback to Svelte would pass every other check. The panel is opened and the page goes back to the
  // Design tab, which the checks that follow start from.
  await check('the Activity panel is drawn by the copy that was asked for', async () => {
    await tab().click();
    await page.locator('[data-testid=activity]').waitFor({ timeout: 10000 });
    assert.equal(await fibre(page, '[data-testid=activity]'), react, react ? 'the Activity panel has no React fibre: the page fell back to Svelte' : 'the Activity panel has a React fibre on the Svelte run');
    await page.click('#right-tab-design');
  });

  const fresh = await list(person);
  if (fresh.batches.length > 0) throw new Error(`the file ${file} already has ${fresh.batches.length} batches: start the server and the pages again on a file name that is new`);
  // The touch ring of a write lasts 1.5 s (and a ring stays while the lane works): sample from before the first write, so the
  // slow calls that follow it cannot make the check miss the window.
  let ringSeen = 0;
  let sampling = true;
  const sampler = (async () => {
    const t0 = Date.now();
    while (sampling && Date.now() - t0 < 20000) { ringSeen = Math.max(ringSeen, await rings()); if (ringSeen > baseRings + 20) return; await sleep(100); }
  })();

  let A, B, C;
  await check('as the agent: two batches with notes get consecutive numbers, a replay and a bad note write nothing, and the review route says forbidden', async () => {
    const who = await tool(agent, 'whoami', {});
    const me = await tool(person, 'whoami', {});
    assert.notEqual(who.actor, me.actor, 'the agent and the person must be different actors');
    assert.equal((await list(person)).canReview, true, 'the person may review');
    assert.equal((await list(agent)).canReview, false, 'the agent may not');
    const a = { idempotencyKey: `act-${run}-a`, note: NOTE_A, commands: [
      { type: 'addNode', node: { id: hero, type: 'frame', name: 'Hero', parentId: null, x: 0, y: 0, w: 480, h: 260, fill: F0 } },
      text(heading, hero, T0, 32, 32), text(blurb, hero, 'One canvas for people and agents.', 90, 16)] };
    A = (await tool(agent, 'apply_commands', a)).batch;
    assert.ok(A?.n >= 1 && A.note === 'kept', `batch A: ${JSON.stringify(A)}`);
    const b = { idempotencyKey: `act-${run}-b`, note: NOTE_B, commands: [
      { type: 'updateNode', id: heading, patch: { text: T1, x: 80 } }, { type: 'updateNode', id: hero, patch: { fill: F1 } }] };
    B = (await tool(agent, 'apply_commands', b)).batch;
    assert.equal(B.n, A.n + 1, `B follows A: ${JSON.stringify(B)}`);
    const seq = await seqNow();
    const again = await tool(agent, 'apply_commands', a);
    assert.equal(again.batch?.n, A.n, 'a replay keeps the number');
    const bad = await tool(agent, 'apply_commands', { idempotencyKey: `act-${run}-bad`, note: 'hidden\u200btext', commands: [{ type: 'updateNode', id: blurb, patch: { text: 'never written' } }] });
    assert.equal(bad.error?.code, 'invalid', `a hidden character in a note: ${JSON.stringify(bad).slice(0, 200)}`);
    assert.equal(await seqNow(), seq, 'the replay and the refused note wrote nothing');
    assert.equal((await list(person)).batches.length >= 2, true);
  });

  await check('the tab counts the unreviewed batches while the Design tab is open, and the touch rings of the agent show on the canvas', async () => {
    const want = (await list(person)).unreviewedCount;
    assert.equal(want, 2);
    await until('the count on the tab', async () => (await badge()) === want);
    await sampler;
    sampling = false;
    assert.ok(ringSeen > baseRings + 20, `no ring: ${ringSeen} agent-colour pixels against ${baseRings} before the writes`);
    await shot(page, 'design-with-ring');
  });

  await check('the Activity tab fetches and shows the agent lane with its numbered batches, newest first, and the notes as literal text', async () => {
    await openActivity();
    await until('the lane', async () => (await page.locator('[data-testid=lane][data-actor="agent:claude"]').count()) === 1);
    const numbers = await page.locator('[data-testid=lane][data-actor="agent:claude"] [data-testid=batch]').evaluateAll((l) => l.map((x) => x.dataset.n));
    assert.deepEqual(numbers, [String(B.n), String(A.n)]);
    assert.match(await row(A.n).locator('[data-testid=batch-status]').innerText(), /^Applied/);
    assert.match(await page.locator('[data-testid=lane][data-actor="agent:claude"] [data-testid=lane-summary]').innerText(), /Agent, .*, 5 changes, 2 batches/);
    const note = row(B.n).locator('[data-testid=batch-note] blockquote');
    assert.equal((await note.innerText()).replace(/[“”]/g, ''), NOTE_B, 'the note is shown as it was written, line break included');
    assert.equal(await note.locator('b, img, script').count(), 0, 'no markup was made from the note');
    assert.ok((await row(B.n).locator('[data-testid=batch-note] figcaption').innerText()).includes('agent:claude'), 'the note names its actor');
    assert.equal(await row(A.n).locator('[data-testid=batch-note] blockquote').innerText(), `“${NOTE_A}”`);
    assert.match(await row(B.n).locator('[data-testid=batch-summary]').innerText(), /2 changed/);
    assert.equal(await page.evaluate(() => window.__stay), true, 'no reload');
    await shot(page, 'activity-lane');
  });

  await check('Show selects the nodes of a batch on the canvas', async () => {
    await row(B.n).locator('[data-testid=batch-show]').click();
    const selected = (nid) => page.locator(`[data-testid=layer][data-id="${nid}"]`).getAttribute('data-selected');
    await until('the nodes of the batch are selected', async () => (await selected(heading)) === 'true' && (await selected(hero)) === 'true');
  });

  await check('the question of a revert takes the focus on Cancel, and Escape closes it and gives the focus back to Revert; so does a restore', async () => {
    await row(B.n).locator('[data-testid=batch-revert]').click();
    await row(B.n).locator('[data-testid=revert-confirm]').waitFor();
    await until('the focus is on Cancel', async () => (await active()) === 'revert-cancel|Cancel');
    await page.keyboard.press('Escape');
    await until('the question is gone', async () => (await row(B.n).locator('[data-testid=revert-confirm]').count()) === 0);
    await until('the focus is back on Revert', async () => (await active()) === 'batch-revert|Revert');
    await row(B.n).locator('[data-testid=batch-revert]').click();
    await row(B.n).locator('[data-testid=revert-cancel]').click();
    await until('Cancel closed it', async () => (await row(B.n).locator('[data-testid=revert-confirm]').count()) === 0);
    await until('the focus is back on Revert after Cancel', async () => (await active()) === 'batch-revert|Revert');
    const first = page.locator('[data-testid=checkpoint]').first();
    await first.locator('[data-testid=checkpoint-restore] button').click();
    await until('the focus is on Cancel of the restore', async () => (await active()) === 'restore-cancel|Cancel');
    await page.keyboard.press('Escape');
    await until('the restore question is gone', async () => (await page.locator('[data-testid=restore-confirm]').count()) === 0);
    await until('the focus is back on Restore', async () => (await active()) === 'checkpoint-restore|Restore');
  });

  await check('the touch rings go out after the lane is quiet', async () => {
    await until('the rings are gone', async () => (await rings()) <= baseRings + 5, 30000);
    await until('the lane is quiet', async () => /quiet|idle/.test((await page.getAttribute('[data-testid=lane][data-actor="agent:claude"]', 'data-state')) ?? ''), 60000);
  });

  await check('a third batch, and the person overwrites a field of the second by hand', async () => {
    C = (await tool(agent, 'apply_commands', { idempotencyKey: `act-${run}-c`, note: NOTE_C, commands: [
      { type: 'addNode', node: { id: card, type: 'frame', name: 'Card', parentId: hero, x: 32, y: 150, w: 160, h: 48, fill: '#e5e7eb' } },
      { type: 'updateNode', id: blurb, patch: { text: 'Written by the third batch' } }] })).batch;
    assert.equal(C.n, B.n + 1);
    await until('the row', async () => (await row(C.n).count()) === 1);
    await page.click('#right-tab-design');
    await page.locator(`[data-testid=layer][data-id="${hero}"] button`).last().click();
    const fill = page.locator('[data-testid=prop-fill]');
    await fill.fill(PERSON);
    await fill.press('Enter');
    await until('the hand edit on the server', async () => (await nodes())[hero].fill === PERSON);
    await openActivity();
    assert.equal((await nodes())[heading].text, T1);
  });

  await check('Accept, Needs review and Clear work for the person, the tab count follows the server, and the row says who changed it', async () => {
    const by = (n, state) => row(n).locator(`[data-testid=review-set][data-state=${state}]`);
    const count = async () => (await list(person)).unreviewedCount;
    assert.equal(await badge(), (await list(person)).unreviewedCount, 'the badge is the server count');
    assert.equal(await badge(), 3);
    await unfold();
    await by(A.n, 'accepted').click();
    await until('A accepted', async () => /Accepted/.test(await row(A.n).locator('[data-testid=batch-review]').innerText()));
    await until('count 2', async () => (await badge()) === (await count()) && (await count()) === 2);
    assert.match(await row(A.n).locator('[data-testid=batch-review-by]').innerText(), /Last changed by person:ada/);
    await by(B.n, 'needs_review').click();
    await until('B flagged', async () => /Needs review/.test(await row(B.n).locator('[data-testid=batch-review]').innerText()));
    assert.equal(await badge(), (await count()), 'the badge is the server count');
    assert.equal(await count(), 2, 'a flagged batch still counts');
    await by(C.n, 'accepted').click();
    await until('count 1', async () => (await badge()) === (await count()) && (await count()) === 1);
    await by(C.n, 'unreviewed').click();
    await until('count 2 again', async () => (await badge()) === (await count()) && (await count()) === 2);
    assert.equal(await by(A.n, 'accepted').count(), 0, 'the state a row has is not offered');
    assert.match(await page.locator('[data-testid=review-count]').innerText(), new RegExp(`^${await count()} batches to review`));
    await shot(page, 'review');
  });

  await check('as the agent, the review route answers forbidden and changes nothing', async () => {
    const before = await list(person);
    const put = await http(agent, 'PUT', `/v1/files/${file}/batches/${A.n}/review`, { state: 'accepted' });
    assert.equal(put.status, 403, JSON.stringify(put));
    assert.equal(put.body?.error?.code, 'forbidden');
    assert.deepEqual((await list(person)).batches, before.batches);
    assert.equal((await list(agent)).canReview, false);
    const seen = (await tool(agent, 'get_batches', {})).batches; // the Read tool of the agent shows the review state read-only
    assert.deepEqual(seen.map((b) => [b.n, b.review]), before.batches.map((b) => [b.n, b.review]));
  });

  if (agentWeb) {
    await check("in the agent's own page the panel shows the batches and no review controls", async () => {
      const mine = await open(pageUrl(agentWeb));
      await openActivity(mine);
      await until('rows', async () => (await mine.locator('[data-testid=batch][data-n]').count()) >= 3);
      assert.equal(await mine.locator('[data-testid=review-set]').count(), 0);
      assert.match(await mine.locator('[data-testid=review-count]').innerText(), new RegExp(`^${(await list(person)).unreviewedCount} batches to review`));
      await shot(mine, 'agent-page');
      await mine.close();
    });
  } else skip("the agent's own page", 'set MELDFRAME_AGENT_WEB');

  const state = async () => { const n = await nodes(); return { text: n[heading].text, x: n[heading].x, fill: n[hero].fill, blurb: n[blurb].text, card: n[card] !== undefined }; };
  const asBefore = { text: T1, x: 80, fill: PERSON, blurb: 'Written by the third batch', card: true };
  const reverted = { text: T0, x: 32, fill: PERSON, blurb: 'Written by the third batch', card: true };
  const revert = async (n) => { await unfold(); await row(n).locator('[data-testid=batch-revert]').click(); await page.locator('[data-testid=revert-confirm-yes]').click(); };
  const undo = async () => { await page.locator('canvas').first().focus(); await page.keyboard.press('Control+z'); };

  await check("Revert of the second batch restores the fields its agent wrote last; the person's colour and the third batch stay; one undo takes it back", async () => {
    assert.deepEqual(await state(), asBefore);
    await revert(B.n);
    await until('the focus is back on Revert of that batch after the revert was confirmed', async () => (await active()) === 'batch-revert|Revert');
    await until('the revert reached the server', async () => (await state()).text === T0);
    assert.deepEqual(await state(), reverted, 'the heading goes back, the hero keeps the green of the person');
    assert.match(await page.locator('[data-testid=activity-message]').innerText(), /Reverted/);
    await shot(page, 'reverted');
    await undo();
    await until('the undo reached the server', async () => (await state()).text === T1);
    assert.deepEqual(await state(), asBefore, 'one undo took the whole revert back');
  });

  await check('the checkpoints of the agent run and of the revert are listed; Restore appends changes and one undo takes it back', async () => {
    const words = await page.locator('[data-testid=checkpoint]').evaluateAll((l) => l.map((x) => x.textContent));
    assert.ok(words.some((t) => /Before .*run/.test(t)), `a run checkpoint: ${words}`);
    assert.ok(words.some((t) => /Before reverting/.test(t)), `a revert checkpoint: ${words}`);
    // A fourth batch the page sees live: a field of the card, so the revert has something to put back that no frame removal is in.
    const D = (await tool(agent, 'apply_commands', { idempotencyKey: `act-${run}-d`, note: 'Recolours the card.', commands: [{ type: 'updateNode', id: card, patch: { fill: '#facc15' } }] })).batch;
    await until('the row', async () => (await row(D.n).count()) === 1);
    const fillOf = async () => (await nodes())[card].fill;
    await revert(D.n);
    await until('the card is grey again', async () => (await fillOf()) === '#e5e7eb');
    const seq = await seqNow();
    const newest = page.locator('[data-testid=checkpoint]').first();
    assert.match(await newest.innerText(), /Before reverting/);
    await newest.locator('[data-testid=checkpoint-restore] button').click();
    const asked = await newest.getAttribute('data-checkpoint');
    await page.locator('[data-testid=restore-yes]').click();
    await until('the focus is back on Restore of that checkpoint after the restore was confirmed', async () => (await active()) === 'checkpoint-restore|Restore' && (await page.evaluate(() => document.activeElement?.closest('[data-checkpoint]')?.getAttribute('data-checkpoint'))) === asked);
    await until('restored', async () => (await fillOf()) === '#facc15');
    assert.deepEqual(await state(), asBefore, 'the restore touched nothing but the card');
    assert.ok((await seqNow()) > seq, 'the restore wrote new changes at the end of the log');
    assert.match(await page.locator('[data-testid=activity-message]').innerText(), /Restored/);
    await undo();
    await until('the restore undone', async () => (await fillOf()) === '#e5e7eb');
  });

  await check('ops sent over REST with batch keys and no header make a row with no number, no note and no review', async () => {
    const ops = [0, 1].map((i) => ({ key: `rest${run}:${i}`, command: { type: 'updateNode', id: blurb, patch: { text: `REST ${i}` } }, deps: [] }));
    const put = await http(agent, 'POST', `/v1/files/${file}/tools/apply_commands`, { ops });
    assert.equal(put.status, 200, JSON.stringify(put.body));
    const bare = page.locator('[data-testid=batch]:not([data-n])');
    await until('the row', async () => { await unfold(); return (await bare.count()) === 1; });
    assert.match(await bare.locator('span').first().innerText(), /Batch without a number/);
    assert.equal(await bare.locator('[data-testid=review-set], [data-testid=batch-review], [data-testid=batch-note]').count(), 0);
  });

  await check('a reload keeps the numbers, the notes and the review states, and the count comes from the server', async () => {
    const { batches: server, unreviewedCount } = await list(person);
    await page.reload();
    await until('live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
    await until('the count on the tab', async () => (await badge()) === unreviewedCount);
    await openActivity();
    await until('the rows', async () => (await page.locator('[data-testid=batch][data-n]').count()) >= 4);
    const shown = await page.locator('[data-testid=batch][data-n]').evaluateAll((l) => l.map((x) => ({ n: Number(x.dataset.n), review: x.querySelector('[data-testid=batch-review]').textContent, note: x.querySelector('[data-testid=batch-note] blockquote')?.textContent })));
    const words = { unreviewed: 'Not reviewed', accepted: 'Accepted', needs_review: 'Needs review' };
    for (const b of server.filter((x) => [A.n, B.n, C.n].includes(x.n))) {
      const s = shown.find((x) => x.n === b.n);
      assert.ok(s, `batch ${b.n} is shown`);
      assert.equal(s.review, words[b.review]);
      assert.equal(s.note.replace(/[“”]/g, ''), b.note);
    }
    assert.deepEqual([A.n, B.n, C.n].map((n) => shown.filter((x) => x.n === n).length), [1, 1, 1]);
    // The page that loaded after the batches holds no ops of them: it says so and offers no Revert (live only, ADR-0021).
    assert.equal(await page.locator('[data-testid=batch-revert]').count(), 0);
    assert.ok(await page.locator('[data-testid=batch-not-revertable]').count() >= 4);
    await shot(page, 'after-reload');
  });

  if (react) {
    await check('with the chunks of the React copy blocked, the Svelte panel is drawn and works', async () => {
      const { ctx, page: blocked } = await openWithChunksBlocked(browser, pageUrl(web), pageErrors);
      await blocked.locator('#right-tab-activity').click();
      await blocked.locator('[data-testid=activity]').waitFor({ timeout: 10000 });
      await unfold(blocked);
      assert.equal(await fibre(blocked, '[data-testid=activity]'), false, 'the panel is the Svelte copy');
      await until('the rows', async () => (await blocked.locator('[data-testid=batch][data-n]').count()) >= 3);
      // It works: a review state set from this page reaches the server (batch C went back to Not reviewed above).
      await row(C.n, blocked).locator('[data-testid=review-set][data-state=accepted]').click();
      await until('C accepted on the server', async () => (await list(person)).batches.find((b) => b.n === C.n)?.review === 'accepted');
      await ctx.close();
    });
  }

  await check('no uncaught error in the pages', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error(`activity-e2e: ${e.message}`); process.exit(1); });
