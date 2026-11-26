// The React copy of the Activity panel (first part: the panel, its lanes and the status line), checked as panel.test.ts checks the
// Svelte one: the markup of a server render, in Node, and the same render of the two copies side by side. Clicks, the fold of a lane,
// the line that appears after a refusal and the timer that clears it are in the browser scripts (activity-e2e.mjs with MELDFRAME_UI=react).
import type { ActivityState, Event } from '@meldframe/protocol';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import ActivityPanelSvelte from '../ActivityPanel.svelte';
import MessageLineSvelte from '../MessageLine.svelte';
import { actorView } from '@meldframe/web-document/fixtures';
import { activity, lane } from '../fixtures';
import { ActivityPanel } from './ActivityPanel';
import { MessageLine } from './MessageLine';

const props = (a: ActivityState, over: Record<string, unknown> = {}) => ({ activity: a, checkpoints: [], canCreate: true, status: 'loaded' as const, self: 'person:ana', dispatch: () => {}, ...over });
const html = (a: ActivityState, over: Record<string, unknown> = {}) => renderToStaticMarkup(<ActivityPanel {...props(a, over)} />);
const svelte = (a: ActivityState, over: Record<string, unknown> = {}) => render(ActivityPanelSvelte as never, { props: props(a, over) as never }).body.replace(/<!--.*?-->/g, '');
// The first part draws the status line and the section Recent work; the rows of a batch and the Checkpoints section are the second part.
const recent = (out: string) => out.slice(0, out.indexOf('</section>') + '</section>'.length);

describe('the React status line', () => {
  it('keeps a live region on the page before there is anything to say, and puts the message into it', () => {
    expect(renderToStaticMarkup(<MessageLine message={null} />)).toBe('<div role="status" data-testid="activity-message"></div>');
    const line = renderToStaticMarkup(<MessageLine message={{ tone: 'warning', text: 'You may not accept or flag batches in this file.' }} />);
    expect(line).toMatch(/^<div role="status" data-testid="activity-message"><p data-tone="warning"[^>]*bg-warning-bg[^>]*>You may not accept/);
    expect(renderToStaticMarkup(<MessageLine message={{ tone: 'info', text: 'Checkpoint made.' }} />)).toMatch(/data-tone="info"[^>]*bg-field[^>]*>Checkpoint made\.</);
  });
  it('does not say again what was there when the panel opened', () => {
    const old: Event[] = [{ type: 'refused', code: 'forbidden' }];
    expect(html(activity(), { events: old })).toMatch(/^<div[^>]*><div role="status" data-testid="activity-message"><\/div>/);
  });
});

describe('the React Activity panel, Recent work', () => {
  it('says what it is for when there is nothing yet', () => {
    const out = html(activity());
    expect(out).toContain('data-testid="activity-empty"');
    expect(out).toContain('aria-label="Recent work"');
    expect(out).not.toContain('data-testid="lane"');
  });
  it('counts the batches to review from the server count, and says Unknown, not zero, when the list did not load', () => {
    expect(html(activity({ unreviewedCount: 12 }))).toContain('data-testid="review-count">12 batches to review<');
    expect(html(activity({ unreviewedCount: 1 }))).toContain('>1 batch to review<');
    expect(html(activity({ unreviewedCount: 0 }))).not.toContain('data-testid="review-count"');
    const out = html(activity({ unreviewedCount: null }));
    expect(out).toMatch(/data-testid="review-count"[^>]*data-unknown="true"[^>]*>Batches to review: Unknown\. The list of batches has not loaded\.</);
    expect(out).not.toContain('0 batches');
  });
  it('draws a lane with its kind, state and counts, the full actor as the title, and opens only the first lane', () => {
    const a = activity({ lanes: [lane({ actor: 'agent:claude', state: 'working', ops: 12, batches: ['k2', 'k1'] }), lane({ actor: 'person:ana', batches: [] })] });
    const out = html(a);
    expect(out).toContain('>Agent, working, 12 changes, 2 batches<');
    expect(out).toContain('>Person, idle, 3 changes<');
    expect(out).toMatch(/title="agent:claude"[^>]*><bdi>claude<\/bdi>/);
    expect(out).toMatch(/title="person:ana"[^>]*><bdi>You<\/bdi>/);
    expect(out.match(/aria-expanded="true"/g)).toHaveLength(1);
    expect(out.match(/aria-expanded="false"/g)).toHaveLength(1);
    expect(out).toContain('aria-controls="lane-body-0"');
    expect(out).toContain('aria-controls="lane-body-1"');
    expect(out.match(/id="lane-body-\d"/g)).toEqual(['id="lane-body-0"']);
  });
  it('shows the runs of a lane that has no batch, as the lane of a person does', () => {
    const run = { startMs: new Date(2026, 10, 20, 10, 41, 0).getTime(), endMs: new Date(2026, 10, 20, 10, 44, 5).getTime(), ops: 12 };
    const out = html(activity({ lanes: [lane({ actor: 'person:ana', ops: 12, batches: [], runs: [run, { ...run, startMs: run.endMs + 1, ops: 1 }] })] }));
    expect(out).toContain('data-testid="lane-runs"');
    expect(out).toContain('>10:41:00 to 10:44:05: 12 changes<');
    expect(out).toContain('to 10:44:05: 1 change<');
  });
  it('offers older batches only when the app layer says there are some', () => {
    expect(html(activity({ hasOlder: true }))).toMatch(/data-testid="older"[^>]*>Show older batches</);
    expect(html(activity({ hasOlder: false }))).not.toContain('data-testid="older"');
  });
  it('draws each tone of the app layer in its own dot and works nothing out from the actor', () => {
    const dot = (tone: 'person' | 'agent1' | 'agent2' | 'agent3') => (html(activity({ lanes: [lane({ view: actorView('agent:x', tone), batches: [] })] })).match(/rounded-pill (bg-[a-z0-9-]+)/) ?? [])[1];
    expect([dot('person'), dot('agent1'), dot('agent2'), dot('agent3')]).toEqual(['bg-person', 'bg-agent-1', 'bg-agent-2', 'bg-agent-3']);
  });
});

// The two copies draw the same elements with the same classes in the same order (a class that is not here is a visible difference).
// What a class cannot say (the real pixels) is checked in a browser by pixels-e2e.mjs with a tolerance of 0.
describe('the React Activity panel against the Svelte one', () => {
  const classes = (out: string) => [...out.matchAll(/class="([^"]*)"/g)].map((m) => m[1]!.split(/\s+/).filter(Boolean).sort().join(' '));
  const attrs = (out: string, name: string) => [...out.matchAll(new RegExp(`\\s${name}="([^"]*)"`, 'g'))].map((m) => m[1]!);
  const NAMES = ['data-testid', 'data-actor', 'data-state', 'data-tone', 'data-unknown', 'role', 'aria-label', 'aria-expanded', 'aria-controls', 'aria-hidden', 'title', 'id', 'type'];
  // The text nodes in order. Svelte keeps a space between two elements where the template had a line break; between flex items that is not drawn.
  const tags = (out: string) => out.split(/<[^>]*>/).filter((t) => t.trim() !== '');
  const same = (a: ActivityState, over: Record<string, unknown> = {}) => {
    const [r, s] = [recent(html(a, over)), recent(svelte(a, over))];
    expect(classes(r)).toEqual(classes(s));
    for (const name of NAMES) expect(attrs(r, name), name).toEqual(attrs(s, name));
    expect(tags(r)).toEqual(tags(s));
  };
  const lanes = [
    lane({ actor: 'agent:claude', state: 'working', ops: 12, batches: [] }),
    lane({ actor: 'person:ana', batches: [], runs: [{ startMs: 1_000_000, endMs: 2_000_000, ops: 4 }] }),
    lane({ actor: 'agent:\u{1F300}x', view: actorView('agent:\u{1F300}x', 'agent3'), state: 'quiet', batches: [] }),
  ];
  it('has the same classes, roles, names, test ids and text with nothing, with an unknown count and with a count', () => {
    same(activity());
    same(activity({ unreviewedCount: null }));
    same(activity({ unreviewedCount: 3 }));
    same(activity({ unreviewedCount: 1 }));
  });
  it('has the same classes, roles, names, test ids and text with lanes, runs and older batches', () => {
    same(activity({ lanes, unreviewedCount: 2, hasOlder: true }));
  });
  it('has the same status line, empty and with a message', () => {
    for (const m of [null, { tone: 'warning' as const, text: 'Nothing to revert.' }, { tone: 'info' as const, text: 'Restored: 2 changes sent. One undo takes them back.' }]) {
      const r = renderToStaticMarkup(<MessageLine message={m} />);
      const s = render(MessageLineSvelte as never, { props: { message: m } as never }).body.replace(/<!--.*?-->/g, '');
      expect(classes(r)).toEqual(classes(s));
      expect(tags(r)).toEqual(tags(s));
      expect(attrs(r, 'data-tone')).toEqual(attrs(s, 'data-tone'));
      expect(attrs(r, 'role')).toEqual(attrs(s, 'role'));
    }
  });
});
