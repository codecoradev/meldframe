// The screenshot retry on a fake page: the one Chromium protocol error is retried (twice at most), everything else is not.
import test from 'node:test';
import assert from 'node:assert/strict';
import { screenshot, PROTOCOL_ERROR } from '../screenshot.mjs';

const protocolError = () => new Error(`page.screenshot: ${PROTOCOL_ERROR}\nCall log: ...`);
/** A page whose screenshot throws what `errors` lists, one per call, then answers `png`. */
const fake = (errors, png = Buffer.from('png')) => {
  const page = { calls: 0, async screenshot() { const e = errors[page.calls++]; if (e) throw e; return png; } };
  return page;
};
const options = (lines) => ({ delayMs: 0, log: (m) => lines.push(m) });

test('a screenshot that works is taken once, without a log line', async () => {
  const page = fake([]), lines = [];
  assert.deepEqual(await screenshot(page, {}, options(lines)), Buffer.from('png'));
  assert.equal(page.calls, 1);
  assert.deepEqual(lines, []);
});

test('the protocol error once, then success: one retry, logged', async () => {
  const page = fake([protocolError()]), lines = [];
  assert.deepEqual(await screenshot(page, {}, options(lines)), Buffer.from('png'));
  assert.equal(page.calls, 2);
  assert.deepEqual(lines, ['retry 1: screenshot protocol error']);
});

test('the protocol error three times fails: two retries, then the error', async () => {
  const page = fake([protocolError(), protocolError(), protocolError()]), lines = [];
  await assert.rejects(screenshot(page, {}, options(lines)), /Unable to capture screenshot/);
  assert.equal(page.calls, 3);
  assert.deepEqual(lines, ['retry 1: screenshot protocol error', 'retry 2: screenshot protocol error']);
});

test('another error is not retried', async () => {
  const page = fake([new Error('page.screenshot: Target page, context or browser has been closed')]), lines = [];
  await assert.rejects(screenshot(page, {}, options(lines)), /has been closed/);
  assert.equal(page.calls, 1);
  assert.deepEqual(lines, []);
});

test('a different protocol error is not retried either', async () => {
  const page = fake([new Error('page.screenshot: Protocol error (Page.navigate): Cannot navigate')]), lines = [];
  await assert.rejects(screenshot(page, {}, options(lines)), /Page\.navigate/);
  assert.equal(page.calls, 1);
});

test('the screenshot options reach the page', async () => {
  const seen = [];
  const page = { async screenshot(o) { seen.push(o); return Buffer.alloc(0); } };
  await screenshot(page, { fullPage: true }, options([]));
  assert.deepEqual(seen, [{ fullPage: true }]);
});
