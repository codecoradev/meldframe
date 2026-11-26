// The React copy of the canvas element (E7.3, S11), checked in Node as the other copies are: the markup of a server render, compared
// with the Svelte copy's render token by token (tags, attributes, classes, words), so that a role, a name, a class or a word that one
// has and the other has not fails here. A server render runs no effect, so the surface (the canvas that the effect makes, a fresh one
// for each mount) is not in it; the Svelte template has it in the markup, so it is taken out of that render before the comparison.
// What a server render cannot show (the pointer, the frame loop, the camera, strict mode, the pixels) is the job of
// editor-e2e.mjs, canvas-e2e.mjs, layout-interaction-e2e.mjs and pixels-e2e.mjs with MELDFRAME_UI=react.
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import { tokens } from '@meldframe/web-chrome/react/markup.testutil';
import CanvasSvelte from '../Canvas.svelte';
import { CANVAS_FAILED } from '../api';
import { Canvas, CanvasView, handleOf } from './Canvas';

const scene = { space: 'parent' as const, nodes: [] };
const svelteHtml = (props: Record<string, unknown> = {}) => render(CanvasSvelte as never, { props: { scene, ...props } as never }).body.replace(/<canvas\b[^>]*><\/canvas>/g, '');
const reactHtml = (props: Record<string, unknown> = {}) => renderToStaticMarkup(createElement(Canvas, { scene, ...props }));

describe('the element of the React canvas', () => {
  it('is the host of the Svelte canvas: a focusable application with a name, the canvas class, and no renderer yet', () => {
    expect(tokens(reactHtml())).toEqual(tokens(svelteHtml()));
    expect(tokens(reactHtml())).toEqual(['<div aria-label="Canvas" class="canvas-host" role="application" tabindex="0">', '</div>']);
  });

  it('takes its name from `label`, as the Svelte canvas does', () => {
    expect(tokens(reactHtml({ label: 'File design' }))).toEqual(tokens(svelteHtml({ label: 'File design' })));
    expect(reactHtml({ label: 'File design' })).toContain('aria-label="File design"');
  });

  it('shows the renderer that started in data-renderer, and says in one sentence when the start failed', () => {
    const view = (renderer: string, failed: boolean) => tokens(renderToStaticMarkup(createElement(CanvasView, { label: 'Canvas', renderer, failed, hostRef: null })));
    expect(view('webgl', false)).toEqual(['<div aria-label="Canvas" class="canvas-host" data-renderer="webgl" role="application" tabindex="0">', '</div>']);
    expect(view('', true)).toEqual(['<div aria-label="Canvas" class="canvas-host" role="application" tabindex="0">', '<p class="canvas-failed">', CANVAS_FAILED, '</p>', '</div>']);
  });

  it('says it with the words of the Svelte canvas, which takes them from the same place', () => {
    const source = readFileSync(new URL('../Canvas.svelte', import.meta.url), 'utf8');
    expect(source).toContain('{CANVAS_FAILED}');
    expect(source).not.toContain('could not start in this browser');
    expect(CANVAS_FAILED).toBe('The canvas could not start in this browser.');
  });
});

describe('what a host may call on the React canvas', () => {
  it('is the seven methods of the Svelte canvas, each handed to the driver', () => {
    const calls: string[] = [];
    const driver = new Proxy({}, { get: (_t, name: string) => (...args: unknown[]) => (calls.push(`${name}(${args.join(',')})`), name === 'centre' ? { x: 1, y: 2 } : undefined) });
    const handle = handleOf(driver as never);
    handle.fitAll();
    handle.fitSelection();
    expect(handle.centre()).toEqual({ x: 1, y: 2 });
    handle.focus();
    handle.captureBox('a');
    void handle.capture({ x: 0, y: 0, w: 1, h: 1 }, 2);
    handle.stats();
    expect(calls).toEqual(['fitAll()', 'fitSelection()', 'centre()', 'focus()', 'captureBox(a)', 'capture([object Object],2)', 'stats()']);
    const exported = [...readFileSync(new URL('../Canvas.svelte', import.meta.url), 'utf8').matchAll(/export function (\w+)/g)].map((m) => m[1]).sort();
    expect(Object.keys(handle).sort()).toEqual(exported);
  });
});
