import { describe, expect, it } from 'vitest';
import type { ExportDialog, ExportFinding } from '@meldframe/protocol';
import { PREVIEW_LINES, errorText, fileOf, findingRows, plain, previewOf, reviewText, sizeText } from './view';

const dialog = (over: Partial<ExportDialog> = {}): ExportDialog => ({
  frame: 'f1', name: 'Landing Desktop', format: 'tailwind', html: '<p>hi</p>', css: ':root{}', cssFile: 'site.css', htmlBytes: 9, cssBytes: 7, countedBytes: 16, limit: 262144,
  unsupported: [], warnings: [], assets: [], ...over,
});

describe('the preview', () => {
  it('shows the first lines and says how many there are', () => {
    const text = Array.from({ length: PREVIEW_LINES + 50 }, (_, i) => `line ${i + 1}`).join('\n');
    const p = previewOf(text);
    expect(p.lines).toHaveLength(PREVIEW_LINES);
    expect(p.lines[0]).toBe('line 1');
    expect(p.total).toBe(PREVIEW_LINES + 50);
  });
  it('shows a short document whole, with no trailing empty line', () => {
    expect(previewOf('a\nb\n')).toEqual({ lines: ['a', 'b'], total: 2 });
    expect(previewOf('')).toEqual({ lines: [], total: 0 });
  });
});

describe('what a file of the dialog is', () => {
  it('is the HTML under the name of the frame, and the stylesheet under the name the HTML links', () => {
    expect(fileOf(dialog(), 'html')).toEqual({ name: 'Landing Desktop.html', text: '<p>hi</p>', mime: 'text/html', bytes: 9 });
    // The name comes from the app layer, which knows what the document links: nothing here repeats it.
    expect(fileOf(dialog(), 'css')).toEqual({ name: 'site.css', text: ':root{}', mime: 'text/css', bytes: 7 });
  });
  it('cleans the name as a PNG name is cleaned', () => {
    expect(fileOf(dialog({ name: 'a/b' }), 'html').name).toBe('a-b.html');
  });
});

describe('sizes and words', () => {
  it('writes a size in bytes, KiB or MiB', () => {
    expect(sizeText(900)).toBe('900 bytes');
    expect(sizeText(12_700)).toBe('12.4 KiB');
    expect(sizeText(262_144)).toBe('256.0 KiB');
    expect(sizeText(3 * 1024 * 1024)).toBe('3.0 MiB');
  });
  it('words the refusal for a frame that is too large, with both sizes, and says nothing otherwise', () => {
    expect(errorText(dialog())).toBeNull();
    const text = errorText(dialog({ error: 'E_EXPORT_TOO_LARGE', html: '', css: '', htmlBytes: 0, cssBytes: 0, countedBytes: 400_000 }))!;
    expect(text).toContain('too large');
    expect(text).toContain('390.6 KiB');
    expect(text).toContain('256.0 KiB');
    expect(errorText(dialog({ error: 'E_SOMETHING' }))).toContain('E_SOMETHING');
  });
  it('asks to review first only when batches wait', () => {
    expect(reviewText(0)).toBeNull();
    expect(reviewText(1)).toBe('1 batch is not reviewed yet. Export anyway or review first.');
    expect(reviewText(2)).toBe('2 batches are not reviewed yet. Export anyway or review first.');
  });

  it('says the review state is unknown when the count of batches is unknown, and does not read it as nothing waiting', () => {
    const unknown = reviewText(null)!;
    expect(unknown).toContain('not known');
    expect(unknown).toContain('Export anyway or review first');
    expect(unknown).not.toContain('0 batches');
    expect(reviewText(null)).not.toBeNull();
    expect(reviewText(0)).toBeNull();
  });
  it('lists findings with the name of the node, or the file when there is none', () => {
    const list: ExportFinding[] = [
      { code: 'on_text', node: 't1', nodeName: 'Hero Title', message: 'a text node paints no shadows; it was left out' },
      { code: 'unknown_mode', message: 'a mode that does not exist' },
    ];
    expect(findingRows(list)).toEqual([
      { key: 'on_text:t1:0', node: 'Hero Title', code: 'on_text', message: 'a text node paints no shadows; it was left out' },
      { key: 'unknown_mode::1', node: 'This file', code: 'unknown_mode', message: 'a mode that does not exist' },
    ]);
  });
  it('keeps text from the file to what a person can read: no control characters, no characters that reorder text', () => {
    expect(plain('a\u202eb\u2066c\u200fd\u0007e\u200bf')).toBe('abcdef');
    expect(plain('<b>Hero</b> Title')).toBe('<b>Hero</b> Title'); // markup is not removed here: the view draws it as text
    expect(findingRows([{ code: 'x\u202e', node: 'n', nodeName: 'a\u202eb', message: 'm\u2067' }])[0]).toMatchObject({ node: 'ab', code: 'x', message: 'm' });
    expect(findingRows([{ code: 'x', node: 'n', nodeName: '\u202e', message: 'm' }])[0]!.node).toBe('n');
  });
  it('falls back to the node id when the name is empty', () => {
    expect(findingRows([{ code: 'x', node: 'n9', nodeName: '', message: 'm' }])[0]!.node).toBe('n9');
  });
});
