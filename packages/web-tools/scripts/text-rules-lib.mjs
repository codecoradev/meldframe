// The rule of the view packages about text (U9, thin-UI audit finding T1): a screen does not clean text. Text from a model,
// a server or a file is made safe to show once, where it enters the app layer (`meldframe-text`, Rust), so no view or
// model package may carry a definition of "hidden characters": a Unicode property class of controls, formats or
// separators (`\p{Cf}`, `\p{Cc}`, `\p{Z...}`), the escape of a character that is hidden or reorders a line (U+200B to
// 200F, U+202A to 202E, U+2060 to 206F, the soft hyphen, the fillers, the braille blank...), or such a character
// written out in the source. This is the pure part; `text-rules.test.mjs` runs it on the tree.

/** The Unicode property classes of hidden text (`\p{Cf}`, `\p{Cc}`, `\p{Z}`...), as a regex source or a string holds them. */
const PROPERTY = String.raw`\\{1,2}p\{(?:(?:gc|General_Category)=)?(?:C[a-z]?|Z[a-z]?|Control|Format|Other|Separator|Line_Separator|Paragraph_Separator|Space_Separator|Default_Ignorable_Code_Point)\}`;

/**
 * The code points a screen would be cleaning: hidden by the app layer's one set (`meldframe_text::is_hidden_text`, the controls
 * included). This table is a COPY of that Rust function: it can drift when the set changes. `text-rules.test.mjs` reads
 * `crates/core/meldframe-text/src/charset.rs` and fails when the two differ, so a change there is a change here.
 */
export const HIDDEN_RANGES = [
  [0x0000, 0x001f], [0x007f, 0x009f],
  [0x00ad, 0x00ad], [0x034f, 0x034f], [0x0600, 0x0605], [0x061c, 0x061c], [0x06dd, 0x06dd], [0x070f, 0x070f],
  [0x0890, 0x0891], [0x08e2, 0x08e2],
  [0x115f, 0x1160], [0x17b4, 0x17b5], [0x180b, 0x180f], [0x200b, 0x200f], [0x2028, 0x202e], [0x2060, 0x206f],
  [0x2800, 0x2800], [0x3164, 0x3164], [0xfe00, 0xfe0f], [0xfeff, 0xfeff], [0xffa0, 0xffa0], [0xfff9, 0xfffb],
  [0x110bd, 0x110bd], [0x110cd, 0x110cd], [0x13430, 0x1343f], [0x1bca0, 0x1bca3], [0x1d173, 0x1d17a],
  [0xe0000, 0xe007f], [0xe0100, 0xe01ef],
];
const HIDDEN = HIDDEN_RANGES;
const isHidden = (cp) => HIDDEN.some(([a, b]) => cp >= a && cp <= b);

/** A character built from a code (`String.fromCharCode(0x200b)`): the numbers of a literal call are read like an escape. */
const BUILT = /\bfromC(?:har|ode)(?:Code|Point)\(([^)]*)\)/g;

/** A split escape (`'\\u' + '200b'`, `'\\u{20' + '0b}'`): a `\u` that is not followed by four hex digits or a braced number on the same line. */
const SPLIT = /\\{1,2}u(?:\{(?![0-9a-f]{1,6}\})|(?![0-9a-f{]))/i;

/** An escape of such a code point: four hex digits after a backslash and u, or braces around up to six, or two after a backslash and x (below U+0100 only the soft hyphen counts). */
const ESCAPE = /\\u\{([0-9a-f]{1,6})\}|\\u([0-9a-f]{4})|\\x([0-9a-f]{2})/gi;

/**
 * The 1-based lines of `text` that define or apply a hidden-character class: a property class, an escape of a hidden
 * code point, or such a character written out. Comments are not blanked: a comment that holds one is a literal too, and
 * should say the name of the character instead.
 */
export function hiddenTextLines(text) {
  const lines = [];
  const property = new RegExp(PROPERTY, 'i');
  text.split('\n').forEach((line, i) => {
    let hit = property.test(line);
    for (const m of line.matchAll(ESCAPE)) {
      const cp = parseInt(m[1] ?? m[2] ?? m[3], 16);
      if (isHidden(cp)) hit = true;
    }
    for (const m of line.matchAll(BUILT)) {
      // A call with a literal number says which character it builds; a call with anything else can build any: both count.
      const args = m[1].split(',').map((x) => x.trim());
      if (args.some((x) => !/^(?:0x[0-9a-f]+|\d+)$/i.test(x) || isHidden(Number(x)))) hit = true;
    }
    if (SPLIT.test(line)) hit = true;
    for (const ch of line) {
      const cp = ch.codePointAt(0);
      // A tab and a carriage return are the layout of a source file, not a hidden character written in it.
      if (isHidden(cp) && cp !== 0x09 && cp !== 0x0d) hit = true;
    }
    if (hit) lines.push(i + 1);
  });
  return lines;
}

/** Whether a path is product source of a view, model or host package: not a test, not a script, not the tooling. */
export function isProductSource(path) {
  return (
    /(?:^|\/)(?:packages\/web-[a-z-]+|apps\/web)\/src\//.test(path) &&
    !/\.test\.(?:ts|tsx|js|mjs)$/.test(path) &&
    !/(?:^|\/)packages\/web-tools\//.test(path) &&
    /\.(?:ts|tsx|svelte|js|mjs)$/.test(path)
  );
}

/**
 * The findings of a set of files, after the allow-list: `{ path, line }` for each line that holds a hidden-character
 * class and is not allowed, and an error for an entry that is no longer needed or does not equal its count (entries only
 * go down). `allow` is `[{ file, count, reason }]`.
 */
export function checkText(files, allow) {
  const found = new Map();
  for (const f of files) {
    const lines = hiddenTextLines(f.text);
    if (lines.length) found.set(f.path, lines);
  }
  const problems = [];
  const allowed = new Map(allow.map((a) => [a.file, a]));
  for (const [path, lines] of found) {
    const a = allowed.get(path);
    if (!a) problems.push(...lines.map((line) => `${path}:${line}: a hidden-character class in a view package: text is made safe in the app layer, a screen does not clean it (U9)`));
    else if (a.count !== lines.length) problems.push(`${path}: the allow-list says ${a.count} and the file has ${lines.length}: an entry must equal the count and only goes down`);
  }
  for (const a of allow) {
    if (!found.has(a.file)) problems.push(`${a.file}: allowed but clean: delete the entry`);
  }
  return problems;
}
