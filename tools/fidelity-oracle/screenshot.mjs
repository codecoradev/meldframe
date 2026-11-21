// The one place the harness takes a screenshot. Chromium now and then answers `Page.captureScreenshot` with "Unable to capture
// screenshot" after every numeric gate has passed; the same call a moment later works. That exact protocol error is retried,
// twice at most, with a log line each time so a recurring flake stays visible. A gate failure or any other error is never retried.
export const PROTOCOL_ERROR = 'Protocol error (Page.captureScreenshot): Unable to capture screenshot';
const RETRIES = 2;

/** `page.screenshot(options)`, retried up to RETRIES more times on exactly PROTOCOL_ERROR. */
export async function screenshot(page, options, { delayMs = 500, log = (m) => console.log(m) } = {}) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await page.screenshot(options);
    } catch (e) {
      if (attempt > RETRIES || !String(e?.message).includes(PROTOCOL_ERROR)) throw e;
      log(`retry ${attempt}: screenshot protocol error`);
      await new Promise((r) => setTimeout(r, delayMs));
    }
  }
}
