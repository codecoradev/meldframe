import { describe, expect, it } from 'vitest';
import { EXPORT_MAX_PIXELS, EXPORT_SCALES, exportFileName, planExport } from './export';

describe('exportFileName', () => {
  it('uses the layer name, with the scale for 2x', () => {
    expect(exportFileName('Hero', 'n1', 1)).toBe('Hero.png');
    expect(exportFileName('Hero', 'n1', 2)).toBe('Hero@2x.png');
  });
  it('replaces what a file system refuses and keeps no leading dot', () => {
    expect(exportFileName('a/b\\c:d*e?f"g<h>i|j', 'n1', 1)).toBe('a-b-c-d-e-f-g-h-i-j.png');
    expect(exportFileName('..hidden', 'n1', 1)).toBe('hidden.png');
    expect(exportFileName('tab\there', 'n1', 1)).toBe('tab-here.png');
  });
  it('falls back to the id, then to a fixed name', () => {
    expect(exportFileName('   ', 'frame-7', 1)).toBe('frame-7.png');
    expect(exportFileName('', '', 2)).toBe('export@2x.png');
  });
  it('keeps clear of the names Windows reserves', () => {
    expect(exportFileName('CON', 'n1', 1)).toBe('CON-.png');
    expect(exportFileName('nul', 'n1', 2)).toBe('nul-@2x.png');
    expect(exportFileName('Console', 'n1', 1)).toBe('Console.png');
    expect(exportFileName('NUL.tar', 'n1', 1)).toBe('NUL-.tar.png');
    expect(exportFileName('con.backup.v2', 'n1', 2)).toBe('con-.backup.v2@2x.png');
  });
  it('cuts a long name to 80 characters', () => {
    expect(exportFileName('x'.repeat(200), 'n1', 1)).toBe(`${'x'.repeat(80)}.png`);
  });
});

describe('planExport', () => {
  it('offers 1x and 2x and doubles the pixels at 2x', () => {
    expect(EXPORT_SCALES).toEqual([1, 2]);
    expect(planExport({ x: 0, y: 0, w: 100.5, h: 50 }, 2)).toEqual({ ok: true, scale: 2, width: 201, height: 100 });
  });
  it('refuses a node that is not there or too large', () => {
    expect(planExport(null, 1)).toEqual({ ok: false, code: 'node_not_found' });
    const side = Math.sqrt(EXPORT_MAX_PIXELS);
    expect(planExport({ x: 0, y: 0, w: side, h: side }, 1).ok).toBe(true);
    expect(planExport({ x: 0, y: 0, w: side, h: side }, 2)).toEqual({ ok: false, code: 'too_large' });
  });
});
