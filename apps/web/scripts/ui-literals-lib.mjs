// The literal lint for the web client (U4), the TypeScript and Svelte side of `cargo xtask ui-literals`:
// colours, font sizes and families, durations and pixel sizes live in src/tokens.ts and nowhere else.

const HEX = /(?<![\w&])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?!\w)/;
const HEX_NUMBER = /\b0x[0-9a-fA-F]{3,8}\b/;
const NAMES =
  'aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen';
const NAMED_COLOUR = new RegExp(`(?:^|[;{\\s])(?:color|background(?:-color)?|border(?:-[a-z]+)?-color|border|outline(?:-color)?|fill|stroke|caret-color|accent-color)\\s*:[^;}]*?(?<![\\w-])(?:${NAMES.split(' ').join('|')})(?![\\w-])`, 'i');
const COLOUR_FN = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(/;
const FONT_CSS = /(?<![\w-])font(?:-size|-family)?\s*:\s*(?!var\(|inherit|initial|unset)(?=\S)/;
const FONT_TS = /\bfont(?:Size|Family)\s*:\s*['"\d]/;
const DURATION = /(?<![\w.-])\d*\.?\d+(?:ms|s)\b/;
const DURATION_CONTEXT = /transition|animation|duration|delay/;
const PIXELS = /(?<![\w.-])(?:0*[1-9]\d*|\d*\.\d*[1-9]\d*)(?:px|rem|em|vh|vw|pt)\b/;

/** True for the tokens module itself and for tests, which may hold literals. */
export function isExempt(path) {
  return path === 'src/tokens.ts' || path === 'src/tokens.css' || /\.test\.(?:ts|mjs)$/.test(path);
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
      if (HEX.test(line) || HEX_NUMBER.test(line) || COLOUR_FN.test(line) || NAMED_COLOUR.test(line)) add('colour');
      const font = FONT_CSS.test(line) || FONT_TS.test(line);
      if (font) add('font');
      if (DURATION_CONTEXT.test(line) && DURATION.test(line)) add('duration');
      if (!font && PIXELS.test(line)) add('size');
    });
  return out;
}
