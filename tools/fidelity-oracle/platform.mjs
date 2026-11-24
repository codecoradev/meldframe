// Where the oracle's gate holds. The numbers it checks (a text box within 0.05 px of the core's rect, an ink edge within 1 px) are
// the Linux ones: the text table is recorded by the Chromium of the lockfile and the page is laid out by the same browser with the
// fonts of that machine. On macOS the same code gives text boxes that are whole pixels off (issue 231; the cause is not found), so a
// failure there says nothing about a change. The oracle therefore stops on another platform, and says what to do, unless
// MELDFRAME_ORACLE_ANY_PLATFORM=1.

export const GATE_PLATFORM = 'linux';
export const ANY_PLATFORM = 'MELDFRAME_ORACLE_ANY_PLATFORM';

/** The platform the oracle takes itself to be on: the real one. The test of the guard names another one in
 *  `MELDFRAME_ORACLE_TEST_PLATFORM`, to see a refusal on Linux; the value `linux` is not taken from it, so it cannot let a run
 *  through anywhere. */
export function detectedPlatform(env = process.env) {
  const test = env.MELDFRAME_ORACLE_TEST_PLATFORM;
  return test && test !== GATE_PLATFORM ? test : process.platform;
}

/** The note that ends a run on a platform the gate is not defined on, or null when it may run. */
export function platformRefusal(platform = detectedPlatform(), env = process.env) {
  if (platform === GATE_PLATFORM || env[ANY_PLATFORM] === '1') return null;
  return [
    `fidelity oracle: stopped. This is ${platform}, and only Linux is verified (the Chromium and the fonts of the CI job).`,
    `On macOS the text boxes were seen whole pixels off the core with no change to the code (issue 231), and nothing is known for other platforms, so a text failure here cannot be told from a regression.`,
    `Run it on Linux, or open the pull request: its \`oracle\` job runs it. To run it here anyway (as reported in issue 231, the paint probes and the equality of the two forms held on macOS), set ${ANY_PLATFORM}=1.`,
  ].join('\n');
}

/** Ends the process with exit code 2 and the note when the platform is refused. The entry points call it first, before they write or
 *  serve anything, and `launch()` calls it again, so no path starts the browser without it. */
export function stopUnlessGatePlatform() {
  const refusal = platformRefusal();
  if (refusal) {
    console.error(refusal);
    process.exit(2);
  }
}
