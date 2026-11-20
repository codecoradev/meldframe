import { render } from 'svelte/server';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import EmptyState from './EmptyState.svelte';
import ErrorPage from './ErrorPage.svelte';
import NoFrames from './NoFrames.svelte';
import { hasFrames } from './empty';

const html = (c: unknown, props: Record<string, unknown>) => render(TestHost as never, { props: { of: c, props } as never }).body;
const action = createRawSnippet(() => ({ render: () => '<button>Add a frame</button>' }));

describe('no frames yet', () => {
  it('is true for a file with no frame, whatever else it holds', () => {
    expect(hasFrames([])).toBe(false);
    expect(hasFrames([{ type: 'rect' }, { type: 'text' }])).toBe(false);
    expect(hasFrames([{ type: 'rect' }, { type: 'frame' }])).toBe(true);
  });
});

describe('NoFrames', () => {
  it('offers to add a frame once the canvas is ready and the file has none', () => {
    const out = html(NoFrames, { nodes: [{ type: 'rect' }], ready: true, canAdd: true, onAdd() {} });
    expect(out).toContain('No frames yet');
    expect(out).toMatch(/<button[^>]*>(?:<!---->)?Add a frame/);
  });
  it('cannot add a frame while the page is out of step with the server: the button is there but disabled', () => {
    const out = html(NoFrames, { nodes: [], ready: true, canAdd: false, onAdd() {} });
    expect(out).toContain('No frames yet');
    expect(out).toMatch(/<button[^>]*\sdisabled(?![:-])[^>]*>/);
    expect(html(NoFrames, { nodes: [], ready: true, canAdd: true, onAdd() {} })).not.toMatch(/<button[^>]*\sdisabled(?![:-])[^>]*>/);
  });
  it('says nothing before the canvas is ready, and nothing when there is a frame', () => {
    expect(html(NoFrames, { nodes: [], ready: false, onAdd() {} })).not.toContain('No frames yet');
    expect(html(NoFrames, { nodes: [{ type: 'frame' }], ready: true, onAdd() {} })).not.toContain('No frames yet');
  });
});

describe('EmptyState', () => {
  it('is a titled note with its text and its action; it takes no pointer events outside the card', () => {
    const out = html(EmptyState, { title: 'No frames yet', text: 'Press F and drag.', children: action });
    expect(out).toContain('No frames yet');
    expect(out).toContain('Press F and drag.');
    expect(out).toContain('Add a frame');
    expect(out).toContain('pointer-events-none');
    expect(out).toContain('pointer-events-auto');
    expect(out).toContain('data-testid="empty-state"');
  });
  it('works without an action', () => {
    expect(html(EmptyState, { title: 'T', text: 'x' })).not.toContain('<button');
  });
});

describe('ErrorPage', () => {
  const page = { kind: 'no-access', title: 'No access to this file', text: 'Nothing was changed.', detail: '<b>server text</b>', action: 'Try again' };
  it('is the main landmark with a heading, the text and a button', () => {
    const out = html(ErrorPage, { page, onAction() {} });
    expect(out).toMatch(/<main[^>]*data-testid="error-page"[^>]*data-kind="no-access"/);
    expect(out).toMatch(/<h1[^>]*>No access to this file<\/h1>/);
    expect(out).toContain('Nothing was changed.');
    expect(out).toMatch(/<button[^>]*>(?:<!---->)?Try again/);
  });
  it('draws the detail from the server as plain text', () => {
    const out = html(ErrorPage, { page, onAction() {} });
    expect(out).not.toContain('<b>server text');
    expect(out).toContain('&lt;b>server text&lt;/b>');
  });
  it('is announced when it replaces the page: a live region around the heading and text, and the heading takes the focus', () => {
    const out = html(ErrorPage, { page, onAction() {} });
    expect(out).toMatch(/role="alert"[^>]*>[\s\S]*<h1[^>]*tabindex="-1"/);
    expect(out).toMatch(/<main[^>]*>/);
    expect(out).not.toMatch(/<main[^>]*role=/);
  });
  it('has no button and no detail block when there is neither', () => {
    const out = html(ErrorPage, { page: { kind: 'start', title: 'T', text: 'x' }, onAction() {} });
    expect(out).not.toContain('<button');
    expect(out).not.toContain('data-testid="error-detail"');
  });
});
