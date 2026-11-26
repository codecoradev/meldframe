// What the tests of the React copies compare in the markup of a Svelte and a React render of the same state (not used by the product):
// every tag with its classes (sorted: a class that is not on both is a visible difference), the attributes that name, label or
// state an element, the boolean attributes it has, and the text that stands between the tags. The two renderers write some things
// differently, and those are normalised: a boolean attribute is `disabled` in one and `disabled=""` in the other, `style` has
// spaces and a semicolon in one, comments (`<!---->`) are Svelte's own, and the ids that the two libraries make for a tooltip
// (`bits-s1`, `base-ui-_R_va_`) are theirs. Left out as no difference: `tabindex="0"` on a button, link or field (Bits UI's tooltip trigger
// writes it, Base UI's does not; the element is in the tab order either way) and the `<link rel="preload">` that React 19 writes before the
// markup of a server render for an image.

export interface TagView {
  tag: string;
  classes: string;
  /** `name=value` for the attributes that name or state an element. */
  attrs: string[];
  /** The boolean attributes the tag has. */
  flags: string[];
}

export interface MarkupView {
  tags: TagView[];
  /** The text between the tags, trimmed, without the empty runs. */
  texts: string[];
}

const VALUED = ['id', 'for', 'role', 'type', 'name', 'data-testid', 'data-id', 'data-field', 'aria-label', 'aria-labelledby', 'aria-describedby', 'aria-checked', 'aria-expanded', 'aria-pressed', 'aria-current', 'aria-haspopup', 'aria-disabled', 'aria-invalid', 'title', 'placeholder', 'value', 'src', 'alt', 'href', 'tabindex', 'style'];
const FLAGS = ['disabled', 'checked', 'selected', 'readonly', 'required', 'hidden'];

function attributes(raw: string): { valued: Map<string, string>; flags: Set<string> } {
  const valued = new Map<string, string>();
  const flags = new Set<string>();
  for (const m of raw.matchAll(/([a-zA-Z_:][\w:.-]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g)) {
    const name = m[1]!.toLowerCase();
    const value = m[2] ?? m[3] ?? m[4];
    // `disabled=""` is a flag, and so is the bare word; `aria-disabled="true"` is a value.
    if (FLAGS.includes(name)) flags.add(name);
    else if (value !== undefined) valued.set(name, value);
  }
  return { valued, flags };
}

const decode = (t: string) => t.replace(/&#x27;|&#39;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');

export function viewOf(html: string): MarkupView {
  const clean = decode(html.replace(/<!--[\s\S]*?-->/g, ''));
  const tags: TagView[] = [];
  for (const m of clean.matchAll(/<([a-zA-Z][\w-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g)) {
    // React 19 puts a `<link rel="preload">` before the markup of a server render for each image: not part of the element tree.
    if (m[1]!.toLowerCase() === 'link') continue;
    const { valued, flags } = attributes(m[2]!);
    const attrs = VALUED.filter((n) => valued.has(n))
      .map((n) => [n, valued.get(n)!] as const)
      .filter(([n, v]) => !(n === 'id' && /^(bits|base-ui)-/.test(v)))
      // `tabindex="0"` on a button, link or field is what it is without it (Bits UI's tooltip trigger writes it, Base UI's does not).
      .filter(([n, v]) => !(n === 'tabindex' && v === '0' && /^(button|a|input|select|textarea)$/i.test(m[1]!)))
      .map(([n, v]) => `${n}=${n === 'style' ? v.replace(/[\s;]/g, '') : v}`);
    tags.push({
      tag: m[1]!.toLowerCase(),
      classes: (valued.get('class') ?? '').split(/\s+/).filter(Boolean).sort().join(' '),
      attrs,
      flags: FLAGS.filter((f) => flags.has(f)),
    });
  }
  const texts = clean.split(/<[^>]*>/).map((t) => t.replace(/\s+/g, ' ').trim()).filter(Boolean);
  return { tags, texts };
}
