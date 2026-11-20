import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { PRESET_ORDER, presetAction, presetItems, presetSize, PRESETS } from './presets';

// The sizes and names are the app layer's: they are read from `FramePreset` in the Rust source, so a size changed there
// and not here fails this test (as `web-activity` reads the refusal codes from the Rust sources).
const rust = readFileSync(new URL('../../../crates/core/meldframe-app-model/src/action.rs', import.meta.url), 'utf8');
const camel = (v: string) => v.charAt(0).toLowerCase() + v.slice(1);
const block = (start: string) => rust.slice(rust.indexOf(start), rust.indexOf('\n    }', rust.indexOf(start)));
const ALL = [...block('pub const ALL: [FramePreset;').matchAll(/FramePreset::(\w+),/g)].map((m) => camel(m[1]!));
const SIZES = new Map([...block('pub fn size(self)').matchAll(/FramePreset::(\w+) => \((\d+)\.0, (\d+)\.0\)/g)].map((m) => [camel(m[1]!), [Number(m[2]), Number(m[3])]]));
const NAMES = new Map([...block('pub fn name(self)').matchAll(/FramePreset::(\w+) => "([^"]+)"/g)].map((m) => [camel(m[1]!), m[2]!]));
const TABLE = ALL.map((id) => [id, NAMES.get(id), ...(SIZES.get(id) ?? [])] as const);

describe('the Rust table the presets are read from', () => {
  it('was found: every preset of the menu has a name and a size', () => {
    expect(ALL.length).toBeGreaterThanOrEqual(7);
    expect(SIZES.size).toBe(ALL.length);
    expect(NAMES.size).toBe(ALL.length);
    for (const [id, name, w, h] of TABLE) expect([id, typeof name, typeof w, typeof h]).toEqual([id, 'string', 'number', 'number']);
  });
});

describe('presetAction', () => {
  it('centres the new frame on the point and makes it a top-level frame', () => {
    expect(presetAction('mobile', { x: 1000, y: 500 })).toEqual({ type: 'createPreset', preset: 'mobile', x: 805, y: 78, parent: null });
    expect(presetAction('desktop', { x: 0, y: 0 })).toEqual({ type: 'createPreset', preset: 'desktop', x: -720, y: -450, parent: null });
    expect(presetAction('a4', { x: 0, y: 0 })).toEqual({ type: 'createPreset', preset: 'a4', x: -297, y: -421, parent: null });
  });
  it('rounds to whole units so the frame sits on the pixel grid', () => {
    const a = presetAction('tablet', { x: 100.4, y: 99.6 });
    expect(Number.isInteger((a as { x: number }).x) && Number.isInteger((a as { y: number }).y)).toBe(true);
  });
});

describe('the preset table', () => {
  it('matches the names and sizes of the app layer, in the order of its menu', () => {
    expect(PRESET_ORDER.map((id) => [id, PRESETS[id].name, PRESETS[id].w, PRESETS[id].h])).toEqual(TABLE.map((r) => [...r]));
    for (const [id, , w, h] of TABLE) expect(presetSize(id as keyof typeof PRESETS)).toEqual({ w, h });
  });
  it('lists each preset once', () => {
    expect(new Set(PRESET_ORDER).size).toBe(PRESET_ORDER.length);
    expect([...PRESET_ORDER].sort()).toEqual(Object.keys(PRESETS).sort());
  });
});

describe('the rows of the frame menu', () => {
  it('has one row per preset: its name, its size as the trailing text, and the test id the browser checks use', () => {
    const seen: string[] = [];
    const rows = presetItems((p) => seen.push(p));
    expect(rows.map((r) => [r.id, r.label, r.shortcut, r.testId])).toEqual(TABLE.map(([id, name, w, h]) => [id, name, `${w} × ${h}`, `preset-${id}`]));
    rows[3]!.onSelect();
    expect(seen).toEqual(['mobile']);
  });
});
