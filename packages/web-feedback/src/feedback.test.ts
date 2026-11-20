import { readFileSync } from 'node:fs';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import BannerStack from './BannerStack.svelte';
import TestHost from '@meldframe/web-chrome/TestHost.svelte';
import Toast from './Toast.svelte';

const html = (c: unknown, props: Record<string, unknown>) => render(TestHost as never, { props: { of: c, props } as never }).body;
const noop = () => {};

describe('Toast', () => {
  it('keeps its live region on the page with nothing in it, so the next message is announced', () => {
    const out = html(Toast, { toast: null, onClose: noop });
    expect(out).toContain('role="status"');
    expect(out).not.toContain('data-testid="notice"');
  });

  it('puts the message in that region, with the tone, and a Dismiss button that has a name', () => {
    const out = html(Toast, { toast: { id: 1, text: 'That node no longer exists.', tone: 'warning' }, onClose: noop });
    expect(out).toMatch(/role="status"[^>]*>[\s\S]*data-testid="notice"[^>]*data-tone="warning"/);
    expect(out).toContain('That node no longer exists.');
    expect(out).toContain('aria-label="Dismiss"');
    expect(out).toContain('bg-warning-bg');
    expect(out).toContain('text-warning');
  });

  it('draws a danger message with the pale error text on the error fill', () => {
    const out = html(Toast, { toast: { id: 2, text: 'x', tone: 'danger' }, onClose: noop });
    expect(out).toContain('bg-danger-bg');
    expect(out).toContain('text-danger-text');
  });

  it('draws the text as text: markup in it is escaped (an error message can come from a server)', () => {
    const out = html(Toast, { toast: { id: 3, text: '<img src=x onerror=alert(1)>', tone: 'warning' }, onClose: noop });
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;img src=x onerror=alert(1)>');
  });

  it('is a new element for every message, so the same text said twice is announced again', () => {
    // A key block on the id of the message: without it the second message only changes an id and the reader hears nothing.
    // (No DOM here: the compiled component is read for the key block; the browser check of the same is in the PR text.)
    const source = readFileSync(new URL('./Toast.svelte', import.meta.url), 'utf8');
    const client = compile(source, { filename: 'Toast.svelte', generate: 'client' }).js.code;
    expect(client).toMatch(/\$\.key\([^,]+, \(\) => \$\$props\.toast\.id/);
  });

  it('does not block the canvas: the region takes no pointer events, only the toast does', () => {
    const out = html(Toast, { toast: { id: 1, text: 'x', tone: 'warning' }, onClose: noop });
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
    const out = html(BannerStack, { banners: [], onAction: noop });
    expect(out).toMatch(/role="alert"[^>]*data-testid="banner-region-alert"/);
    expect(out).toMatch(/role="status"[^>]*data-testid="banner-region-status"/);
    expect(out).not.toContain('data-testid="banner-c');
    expect(out).not.toContain('data-testid="banner-h');
  });

  it('puts an error banner in the alert region and a warning in the status region, never the other way', () => {
    const out = html(BannerStack, { banners: [banner('halted', 'danger', 'Out of step.'), banner('connection', 'warning', 'Offline.')], onAction: noop });
    expect(region(out, 'alert')).toContain('data-testid="banner-halted"');
    expect(region(out, 'alert')).not.toContain('banner-connection');
    expect(region(out, 'status')).toContain('data-testid="banner-connection"');
    expect(region(out, 'status')).not.toContain('banner-halted');
  });

  it('shows the action as a button named by its label', () => {
    const out = html(BannerStack, { banners: [banner('connection', 'warning', 'Offline.', retry)], onAction: noop });
    expect(out).toMatch(/<button[^>]*>(?:<!---->)?Retry now(?:<!---->)?<\/button>/);
  });

  it('a banner without an action has no button', () => {
    expect(html(BannerStack, { banners: [banner('c', 'warning', 'Reconnecting.')], onAction: noop })).not.toContain('<button');
  });

  it('draws the text as text', () => {
    const out = html(BannerStack, { banners: [banner('canvas-error', 'danger', '<script>alert(1)</script>')], onAction: noop });
    expect(out).not.toContain('<script>alert');
    expect(out).toContain('&lt;script>');
  });
});
