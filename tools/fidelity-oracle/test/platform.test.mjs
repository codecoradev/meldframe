// The platform guard: the gate is defined on Linux (issue 231: on macOS the text boxes are whole pixels off with no change to the
// code), so the oracle refuses to run elsewhere unless asked, and says why.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { OUT } from '../lib.mjs';
import { ANY_PLATFORM, GATE_PLATFORM, detectedPlatform, platformRefusal } from '../platform.mjs';

test('Linux runs, with nothing to say', () => {
  assert.equal(GATE_PLATFORM, 'linux');
  assert.equal(platformRefusal('linux', {}), null);
});

test('macOS and Windows are refused, and the refusal says why, where the problem is and how to run anyway', () => {
  for (const platform of ['darwin', 'win32']) {
    const m = platformRefusal(platform, {});
    assert.match(m, new RegExp(platform), 'it names the platform');
    assert.match(m, /Linux/);
    assert.match(m, /issue 231/);
    assert.match(m, /`oracle` job/, 'it points at the CI job that runs the oracle');
    assert.ok(m.includes(`${ANY_PLATFORM}=1`), 'it names the variable that runs it anyway');
  }
});

test('the variable set to 1, and only 1, runs it anyway', () => {
  assert.equal(platformRefusal('darwin', { [ANY_PLATFORM]: '1' }), null);
  for (const v of ['', '0', 'true', 'yes', ' 1']) assert.ok(platformRefusal('darwin', { [ANY_PLATFORM]: v }), `"${v}" is not 1`);
});

test('launch() stops with exit 2 and the message on another platform, before it needs a browser', () => {
  const lib = path.join(path.dirname(fileURLToPath(import.meta.url)), '../lib.mjs');
  const code = `import { launch } from ${JSON.stringify(lib)}; await launch();`;
  const r = spawnSync(process.execPath, ['--input-type=module', '-e', code], { encoding: 'utf8', env: { ...process.env, MELDFRAME_ORACLE_TEST_PLATFORM: 'darwin', [ANY_PLATFORM]: '' } });
  assert.equal(r.status, 2, r.stderr);
  assert.match(r.stderr, /issue 231/);
});

test('the test platform is taken from the variable, except `linux`, which never lets a run through', () => {
  assert.equal(detectedPlatform({ MELDFRAME_ORACLE_TEST_PLATFORM: 'darwin' }), 'darwin');
  assert.equal(detectedPlatform({ MELDFRAME_ORACLE_TEST_PLATFORM: 'linux' }), process.platform);
  assert.equal(detectedPlatform({}), process.platform);
});

const here = path.dirname(fileURLToPath(import.meta.url));
const refused = (entry, args = []) => spawnSync(process.execPath, [path.join(here, '..', entry), ...args], { encoding: 'utf8', env: { ...process.env, MELDFRAME_ORACLE_TEST_PLATFORM: 'darwin', [ANY_PLATFORM]: '' } });

test('oracle.mjs stops first: it leaves the pages of an earlier run in place and serves nothing', () => {
  // Not to be run in the middle of a real oracle run of the same checkout: that run clears out/pages. The name is this process's own.
  const sentinel = path.join(OUT, 'pages', `platform-test-sentinel-${process.pid}`);
  fs.mkdirSync(path.dirname(sentinel), { recursive: true });
  fs.writeFileSync(sentinel, 'x');
  try {
    const r = refused('oracle.mjs', ['--seeds', '1']);
    assert.equal(r.status, 2, r.stderr);
    assert.match(r.stderr, /issue 231/);
    assert.equal(r.stdout, '');
    assert.ok(fs.existsSync(sentinel), 'the guard ran after the pages were cleared');
  } finally {
    fs.rmSync(sentinel, { force: true });
  }
});

test('sensitivity.mjs stops with exit 2 and the message too', () => {
  const r = refused('sensitivity.mjs');
  assert.equal(r.status, 2, r.stderr);
  assert.match(r.stderr, /issue 231/);
  assert.equal(r.stdout, '');
});
