// The literal lint for the web client (U4), the TypeScript and Svelte side of `cargo xtask ui-literals`:
// colours, font sizes and families, durations and pixel sizes live in src/tokens.ts and nowhere else.

const HEX = /(?<![\w&])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?!\w)/;
const HEX_NUMBER = /\b0x[0-9a-fA-F]{6,8}\b/;
const COLOUR_FN = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(/;
const FONT_CSS = /font-(?:size|family)\s*:\s*(?!var\()(?=\S)/;
const FONT_TS = /\bfont(?:Size|Family)\s*:\s*['"\d]/;
const DURATION = /(?<![\w.-])\d*\.?\d+(?:ms|s)\b/;
const DURATION_CONTEXT = /transition|animation|duration|delay/;
const PIXELS = /(?<![\w.-])(?:0*[1-9]\d*|\d*\.\d*[1-9]\d*)px\b/;

/** True for the tokens module itself and for tests, which may hold literals. */
export function isExempt(path) {
  const name = path.split('/').pop() ?? '';
  return name.split('.')[0] === 'tokens' || /\.test\.(?:ts|mjs)$/.test(name);
}

function stripComments(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '))
    .replace(/<!--[\s\S]*?-->/g, (m) => m.replace(/[^\n]/g, ' '))
    .replace(/(^|\s)\/\/.*$/gm, '$1');
}

/** Every literal in `text` (a file at `path`) as `{ path, line, kind }`; kind is colour, font, duration or size. */
export function findLiterals(path, text) {
  const out = [];
  stripComments(text)
    .split('\n')
    .forEach((line, i) => {
      const add = (kind) => out.push({ path, line: i + 1, kind });
      if (HEX.test(line) || HEX_NUMBER.test(line) || COLOUR_FN.test(line)) add('colour');
      const font = FONT_CSS.test(line) || FONT_TS.test(line);
      if (font) add('font');
      if (DURATION_CONTEXT.test(line) && DURATION.test(line)) add('duration');
      if (!font && PIXELS.test(line)) add('size');
    });
  return out;
}
