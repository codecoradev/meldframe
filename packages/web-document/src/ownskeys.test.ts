import { readdirSync, readFileSync, statSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { OWNS_KEYS_SELECTOR, ownsKeys } from './edit';

// A popover, a menu or a dialog of the web client is drawn in a portal, outside the card that owns its keys: the plain keys
// typed in it (an arrow key on a row of the bind picker) must not reach the editor. `ownsKeys` skips what is inside an element
// marked `data-owns-keys` (or a menu or dialog by role), so every portaled content of bits-ui that takes focus carries the mark.
const root = new URL('../../../', import.meta.url).pathname;

function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = `${dir}/${name}`;
    if (name === 'node_modules' || name === 'dist' || name === '.wasm') return [];
    return statSync(path).isDirectory() ? svelteFiles(path) : path.endsWith('.svelte') ? [path] : [];
  });
}
const files = [...readdirSync(`${root}packages`).flatMap((p) => svelteFiles(`${root}packages/${p}`)), ...svelteFiles(`${root}apps/web/src`)];
/** The opening tags of bits-ui contents (not a tooltip: nothing in it takes the focus), as written in the sources. */
const contents = files.flatMap((f) => [...readFileSync(f, 'utf8').matchAll(/<(Popover|Select|Combobox|DropdownMenu|ContextMenu|Dialog|AlertDialog)\.Content\b[^>]*>/gs)].map((m) => ({ file: f.replace(root, ''), tag: m[0] })));

describe('the keys of a portaled popover, menu or dialog stay in it', () => {
  it('finds the contents there are today (so the scan below is not empty)', () => {
    expect(contents.length).toBeGreaterThanOrEqual(5);
    expect(contents.some((c) => c.file.endsWith('BindPicker.svelte'))).toBe(true);
  });
  it('marks every one of them data-owns-keys', () => {
    for (const c of contents) expect(c.tag, c.file).toMatch(/\bdata-owns-keys\b/);
  });
  it('skips a plain key typed on anything inside such a content, and still takes Ctrl and Cmd combinations and Alt', () => {
    const inside = { tagName: 'BUTTON', isContentEditable: false, closest: (sel: string) => (sel === OWNS_KEYS_SELECTOR ? {} : null) };
    const outside = { tagName: 'CANVAS', isContentEditable: false, closest: () => null };
    expect(ownsKeys(inside)).toBe(true);
    expect(ownsKeys(outside)).toBe(false);
    expect(ownsKeys(inside, { ctrlKey: true, metaKey: false })).toBe(false);
    expect(ownsKeys(inside, { ctrlKey: false, metaKey: true })).toBe(false);
    expect(ownsKeys(inside, { ctrlKey: false, metaKey: false, altKey: true })).toBe(false);
  });
  it('lets the selector name the mark', () => expect(OWNS_KEYS_SELECTOR).toContain('[data-owns-keys]'));
});
