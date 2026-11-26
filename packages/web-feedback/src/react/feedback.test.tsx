// The React copies of web-feedback, checked as empty.test.ts and feedback.test.ts check the Svelte ones: the same
// assertions on the markup of a server render (Node, no DOM library). What a browser alone can prove (the live regions
// announcing, the focus on the heading, the pixels) is left to packages/web-feedback/scripts/feedback-e2e.mjs, run on both.
import { readFileSync } from 'node:fs';
import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import { BannerStack } from './BannerStack';
import { EmptyState } from './EmptyState';
import { ErrorPage } from './ErrorPage';
import { FeedbackLayer } from './FeedbackLayer';
import { NoFrames } from './NoFrames';
import { Toast } from './Toast';

const html = (el: ReactElement) => renderToStaticMarkup(<TooltipProvider>{el}</TooltipProvider>);
const noop = () => {};

describe('NoFrames', () => {
  it('offers to add a frame once the canvas is ready and the file has none', () => {
    const out = html(<NoFrames nodes={[{ type: 'rect' }]} ready canAdd onAdd={noop} />);
    expect(out).toContain('No frames yet');
    expect(out).toMatch(/<button[^>]*>Add a frame/);
  });
  it('cannot add a frame while the page is out of step with the server: the button is there but disabled', () => {
    const out = html(<NoFrames nodes={[]} ready canAdd={false} onAdd={noop} />);
    expect(out).toContain('No frames yet');
    expect(out).toMatch(/<button[^>]*\sdisabled(?![:-])[^>]*>/);
    expect(html(<NoFrames nodes={[]} ready canAdd onAdd={noop} />)).not.toMatch(/<button[^>]*\sdisabled(?![:-])[^>]*>/);
  });
  it('says nothing before the canvas is ready, and nothing when there is a frame', () => {
    expect(html(<NoFrames nodes={[]} ready={false} canAdd onAdd={noop} />)).not.toContain('No frames yet');
    expect(html(<NoFrames nodes={[{ type: 'frame' }]} ready canAdd onAdd={noop} />)).not.toContain('No frames yet');
  });
});

describe('EmptyState', () => {
  it('is a titled note with its text and its action; it takes no pointer events outside the card', () => {
    const out = html(
      <EmptyState title="No frames yet" text="Press F and drag.">
        <button type="button">Add a frame</button>
      </EmptyState>,
    );
    expect(out).toContain('No frames yet');
    expect(out).toContain('Press F and drag.');
    expect(out).toContain('Add a frame');
    expect(out).toContain('pointer-events-none');
    expect(out).toContain('pointer-events-auto');
    expect(out).toContain('data-testid="empty-state"');
    expect(out).toContain('aria-label="No frames yet"');
  });
  it('works without an action', () => {
    expect(html(<EmptyState title="T" text="x" />)).not.toContain('<button');
  });
});

describe('ErrorPage', () => {
  const page = { kind: 'no-access' as const, title: 'No access to this file', text: 'Nothing was changed.', detail: '<b>server text</b>', action: 'Try again' };
  it('is the main landmark with a heading, the text and a button', () => {
    const out = html(<ErrorPage page={page} onAction={noop} />);
    expect(out).toMatch(/<main[^>]*data-testid="error-page"[^>]*data-kind="no-access"/);
    expect(out).toMatch(/<h1[^>]*>No access to this file<\/h1>/);
    expect(out).toContain('Nothing was changed.');
    expect(out).toMatch(/<button[^>]*>Try again/);
  });
  it('draws the detail from the server as plain text', () => {
    const out = html(<ErrorPage page={page} onAction={noop} />);
    expect(out).not.toContain('<b>server text');
    expect(out).toContain('&lt;b&gt;server text&lt;/b&gt;');
  });
  it('is announced when it replaces the page: a live region around the heading and text, and the heading can take the focus', () => {
    const out = html(<ErrorPage page={page} onAction={noop} />);
    expect(out).toMatch(/role="alert"[^>]*>[\s\S]*<h1[^>]*tabindex="-1"/);
    expect(out).toMatch(/<main[^>]*>/);
    expect(out).not.toMatch(/<main[^>]*role=/);
  });
  it('has no button and no detail block when there is neither', () => {
    const out = html(<ErrorPage page={{ kind: 'start', title: 'T', text: 'x' }} onAction={noop} />);
    expect(out).not.toContain('<button');
    expect(out).not.toContain('data-testid="error-detail"');
  });
});

describe('Toast', () => {
  it('keeps its live region on the page with nothing in it, so the next message is announced', () => {
    const out = html(<Toast toast={null} onClose={noop} />);
    expect(out).toContain('role="status"');
    expect(out).not.toContain('data-testid="notice"');
  });

  it('puts the message in that region, with the tone, and a Dismiss button that has a name', () => {
    const out = html(<Toast toast={{ id: 1, text: 'That node no longer exists.', tone: 'warning' }} onClose={noop} />);
    expect(out).toMatch(/role="status"[^>]*>[\s\S]*data-testid="notice"[^>]*data-tone="warning"/);
    expect(out).toContain('That node no longer exists.');
    expect(out).toContain('aria-label="Dismiss"');
    expect(out).toContain('bg-warning-bg');
    expect(out).toContain('text-warning');
  });

  it('draws a danger message with the pale error text on the error fill', () => {
    const out = html(<Toast toast={{ id: 2, text: 'x', tone: 'danger' }} onClose={noop} />);
    expect(out).toContain('bg-danger-bg');
    expect(out).toContain('text-danger-text');
  });

  it('draws the text as text: markup in it is escaped (an error message can come from a server)', () => {
    const out = html(<Toast toast={{ id: 3, text: '<img src=x onerror=alert(1)>', tone: 'warning' }} onClose={noop} />);
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;img src=x onerror=alert(1)&gt;');
  });

  it('is a new element for every message, so the same text said twice is announced again', () => {
    // A key on the id of the message: without it the second message only changes an id and the reader hears nothing.
    // The component has no hooks, so it is called as a function and the element it returns is read for its key. Two
    // messages with the same text have different keys, and the key is the id.
    const region = (id: number) => Toast({ toast: { id, text: 'same', tone: 'warning' }, onClose: noop }) as ReactElement<{ children: ReactElement }>;
    expect(region(4).props.children.key).toBe('4');
    expect(region(5).props.children.key).toBe('5');
    // The source says it too, as feedback.test.ts reads the compiled Svelte source for its key block.
    expect(readFileSync(new URL('./Toast.tsx', import.meta.url), 'utf8')).toMatch(/<div key=\{toast\.id\}/);
  });

  it('does not block the canvas: the region takes no pointer events, only the toast does', () => {
    const out = html(<Toast toast={{ id: 1, text: 'x', tone: 'warning' }} onClose={noop} />);
    expect(out).toContain('pointer-events-none');
    expect(out).toContain('pointer-events-auto');
  });
});

describe('BannerStack', () => {
  const retry = { id: 'retry', label: 'Retry now' };
  const banner = (id: string, tone: 'warning' | 'danger', text: string, action?: { id: string; label: string }) => ({ id, tone, text, action });
  /** The markup of the region with `role`: from its tag to the tag of the other one. */
  const region = (out: string, role: 'alert' | 'status') => {
    const [from, to] = role === 'alert' ? ['banner-region-alert', 'banner-region-status'] : ['banner-region-status', ''];
    const start = out.indexOf(from);
    const end = to ? out.indexOf(to) : out.length;
    return out.slice(start, Math.max(start, end) || out.length);
  };

  it('keeps both live regions on the page with nothing in them, so the first banner is announced', () => {
    const out = html(<BannerStack banners={[]} onAction={noop} />);
    expect(out).toMatch(/role="alert"[^>]*data-testid="banner-region-alert"/);
    expect(out).toMatch(/role="status"[^>]*data-testid="banner-region-status"/);
    expect(out).not.toContain('data-testid="banner-c');
    expect(out).not.toContain('data-testid="banner-h');
  });

  it('puts an error banner in the alert region and a warning in the status region, never the other way', () => {
    const out = html(<BannerStack banners={[banner('halted', 'danger', 'Out of step.'), banner('connection', 'warning', 'Offline.')]} onAction={noop} />);
    expect(region(out, 'alert')).toContain('data-testid="banner-halted"');
    expect(region(out, 'alert')).not.toContain('banner-connection');
    expect(region(out, 'status')).toContain('data-testid="banner-connection"');
    expect(region(out, 'status')).not.toContain('banner-halted');
  });

  it('shows the action as a button named by its label', () => {
    const out = html(<BannerStack banners={[banner('connection', 'warning', 'Offline.', retry)]} onAction={noop} />);
    expect(out).toMatch(/<button[^>]*>Retry now<\/button>/);
  });

  it('a banner without an action has no button', () => {
    expect(html(<BannerStack banners={[banner('c', 'warning', 'Reconnecting.')]} onAction={noop} />)).not.toContain('<button');
  });

  it('draws the text as text', () => {
    const out = html(<BannerStack banners={[banner('canvas-error', 'danger', '<script>alert(1)</script>')]} onAction={noop} />);
    expect(out).not.toContain('<script>alert');
    expect(out).toContain('&lt;script&gt;');
  });

  it('draws the tone on the fill, and the tone as data', () => {
    const out = html(<BannerStack banners={[banner('halted', 'danger', 'x'), banner('connection', 'warning', 'y')]} onAction={noop} />);
    expect(out).toMatch(/data-testid="banner-halted" data-tone="danger" class="[^"]*bg-danger-bg text-danger-text/);
    expect(out).toMatch(/data-testid="banner-connection" data-tone="warning" class="[^"]*bg-warning-bg text-warning/);
  });
});

describe('FeedbackLayer', () => {
  it('lays the empty state, the toast and the banners over the canvas, in that order', () => {
    const out = html(
      <FeedbackLayer
        nodes={[]}
        ready
        canAdd
        onAdd={noop}
        toast={{ id: 1, text: 'refused', tone: 'warning' }}
        onCloseToast={noop}
        banners={[{ id: 'connection', tone: 'warning', text: 'Offline.' }]}
        onBanner={noop}
      />,
    );
    const at = (s: string) => out.indexOf(s);
    expect(at('data-testid="empty-state"')).toBeGreaterThan(-1);
    expect(at('data-testid="empty-state"')).toBeLessThan(at('data-testid="toast-region"'));
    expect(at('data-testid="toast-region"')).toBeLessThan(at('data-testid="banner-region-alert"'));
  });
});
