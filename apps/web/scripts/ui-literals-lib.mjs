// The literal lint for the web client (U4), the TypeScript and Svelte side of `cargo xtask ui-literals`
// (this one is the strict check; the xtask is the fast baseline): colours, font sizes and families,
// durations and pixel sizes live in src/tokens.ts and src/tokens.css and nowhere else.

const HEX = /(?<![\w&])#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})(?!\w)/i;
const HEX_NUMBER = /\b0x[0-9a-f]{3,8}\b/i;
const NAMES =
  'aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen';
const NAME_WORD = new RegExp(`(?<![\\w.#-])(?:${NAMES.split(' ').join('|')})(?![\\w-])`, 'i');
const COLOUR_FN = /(?<![\w-])(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|color-mix|light-dark)\(/i;

// Properties and keys that take a colour (or a shorthand that holds one), in CSS and in camelCase.
const KEBAB =
  'color|background(?:-color|-image)?|border(?:-[a-z]+)*|outline(?:-color)?|fill|stroke|caret-color|accent-color|box-shadow|text-shadow|text-decoration(?:-color)?|column-rule(?:-color)?|stop-color|flood-color|filter';
const CAMEL =
  'color|background(?:Color|Image)?|border(?:[A-Z][a-z]+)*|outline(?:Color)?|fill|stroke|caretColor|accentColor|boxShadow|textShadow|textDecoration(?:Color)?|stopColor|floodColor|filter';
const CSS_DECL = new RegExp(`(?<![\\w-])(?:${KEBAB})\\s*:\\s*([^;}"'\\n]*)`, 'gi');
const QUOTED_VALUE = new RegExp(`(?<![\\w-])(?:${KEBAB}|${CAMEL})\\s*[:=]\\s*(['"\`])([^'"\`]*)\\1`, 'gi');
const STYLE_DIRECTIVE = new RegExp(`(?<![\\w-])style:(?:${KEBAB})\\s*=\\s*['"]?([^'"}\\n]*)`, 'gi');
const TW_ARBITRARY = /(?<![\w-])[\w:-]+-\[([^\]\s]+)\]/g;

const FONT_CSS = /(?<![\w-])font(?:-size|-family)?\s*:\s*(?!var\(|inherit|initial|unset)(?=\S)/i;
const FONT_TS = /\bfont(?:Size|Family)\s*:\s*['"\d]/;
const DURATION = /(?<![\w.-])-?\d*\.?\d+(?:ms|s)(?![a-z\d])/i;
const DURATION_CONTEXT = /transition|animation|duration|delay/i;
const BARE_DURATION = /\b(?:duration|delay)\s*:\s*-?\.?\d/;
const PIXELS = /(?<![\w.-])-?(?:0*[1-9]\d*|\d*\.\d*[1-9]\d*)(?:px|rem|em|vh|vw|pt)(?![a-z\d])/i;

/**
 * Files that hold the colours of a sample document (what a design contains), not UI colours. Named one by one
 * by exact path: a new file next to them is linted like any other.
 */
export const DOCUMENT_DATA = ['src/harness/sample.ts'];

/** True for the tokens module itself, the named document-data files and tests, which may hold literals. */
export function isExempt(path) {
  return (
    path === 'src/tokens.ts' ||
    path === 'src/tokens.css' ||
    DOCUMENT_DATA.includes(path) ||
    /\.test\.(?:ts|tsx|js|mts|mjs)$/.test(path)
  );
}

const blank = (s) => s.replace(/[^\n]/g, ' ');

/**
 * Text with comments blanked out (newlines kept, so line numbers hold). Block comments everywhere, `<!-- -->` in
 * markup, `//` comments in script files only outside quoted strings and not as the `//` of a URL.
 */
export function stripComments(text, path = 'a.ts') {
  const lineComments = !/\.(?:css|scss|html|svg|json)$/.test(path);
  const markup = /\.(?:svelte|html|svg)$/.test(path);
  let out = '';
  let quote = '';
  for (let i = 0; i < text.length; ) {
    const c = text[i];
    if (quote) {
      out += c;
      if (c === '\\') {
        out += text[i + 1] ?? '';
        i += 2;
        continue;
      }
      if (c === quote || (c === '\n' && quote !== '`')) quote = '';
      i++;
      continue;
    }
    let end = -1;
    const close = (token) => {
      const at = text.indexOf(token, i + 2);
      return at < 0 ? text.length : at + token.length; // unterminated: blank the rest
    };
    if (c === '/' && text[i + 1] === '*') end = close('*/');
    else if (markup && text.startsWith('<!--', i)) end = close('-->');
    else if (lineComments && c === '/' && text[i + 1] === '/' && text[i - 1] !== ':') {
      end = text.indexOf('\n', i);
      if (end < 0) end = text.length;
    }
    if (end > 0) {
      out += blank(text.slice(i, end));
      i = end;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') quote = c;
    out += c;
    i++;
  }
  return out;
}

const withoutUrls = (v) => v.replace(/url\([^)]*\)/gi, ' ');
const hasName = (v) => NAME_WORD.test(withoutUrls(v));

function hasNamedColour(line) {
  for (const re of [CSS_DECL, STYLE_DIRECTIVE]) {
    for (const m of line.matchAll(re)) if (hasName(m[1])) return true;
  }
  for (const m of line.matchAll(QUOTED_VALUE)) if (hasName(m[2])) return true;
  for (const m of line.matchAll(TW_ARBITRARY)) if (hasName(m[1].replace(/^(?:color|length):/i, '').replace(/_/g, ' '))) return true;
  return false;
}

/** Every literal in `text` (a file at `path`) as `{ path, line, kind }`; kind is colour, font, duration or size. */
export function findLiterals(path, text) {
  const out = [];
  stripComments(text, path)
    .split('\n')
    .forEach((line, i) => {
      const add = (kind) => out.push({ path, line: i + 1, kind });
      if (HEX.test(line) || HEX_NUMBER.test(line) || COLOUR_FN.test(line) || hasNamedColour(line)) add('colour');
      const font = FONT_CSS.test(line) || FONT_TS.test(line);
      if (font) add('font');
      if ((DURATION_CONTEXT.test(line) && DURATION.test(line)) || BARE_DURATION.test(line)) add('duration');
      if (!font && PIXELS.test(line)) add('size');
    });
  return out;
}
