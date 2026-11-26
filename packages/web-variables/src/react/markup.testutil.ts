// A server render as a list of tokens, so that the Svelte copy and the React copy of a component can be compared as the browser
// would see them: for each tag its name and its attributes (sorted by name, the class names sorted, a boolean attribute as
// `name=""`), for each run of text its words. Comments (Svelte's markers), an empty `class` and whitespace between tags are dropped,
// and the entities the two renderers write differently are decoded. For the tests of the React copies; it draws nothing.
const ENTITIES: Record<string, string> = { '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&#x27;': "'", '&#39;': "'" };
const decode = (s: string) => s.replace(/&(?:amp|lt|gt|quot|#x27|#39);/g, (e) => ENTITIES[e] ?? e);

const TOKEN = /<!--[\s\S]*?-->|<(\/?)([a-zA-Z][a-zA-Z0-9-]*)((?:\s+[^\s=>/]+(?:="[^"]*")?)*)\s*\/?>|([^<]+)/g;
const ATTR = /([^\s=>/]+)(?:="([^"]*)")?/g;

export function tokens(html: string): string[] {
  const out: string[] = [];
  for (const m of html.matchAll(TOKEN)) {
    if (m[2] !== undefined) {
      const attrs: string[] = [];
      for (const a of (m[3] ?? '').matchAll(ATTR)) {
        const name = a[1]!;
        const value = decode(a[2] ?? '');
        if (name === 'class') {
          const classes = value.split(/\s+/).filter(Boolean).sort().join(' ');
          if (classes) attrs.push(`class="${classes}"`);
        } else attrs.push(`${name}="${value}"`);
      }
      out.push(`<${m[1]}${m[2]} ${attrs.sort().join(' ')}>`.replace(' >', '>'));
    } else if (m[4] !== undefined) {
      const text = decode(m[4]).replace(/\s+/g, ' ').trim();
      if (text) out.push(text);
    }
  }
  return out;
}
