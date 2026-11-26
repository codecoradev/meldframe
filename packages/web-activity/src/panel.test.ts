import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { ActivityState, Checkpoint } from '@meldframe/protocol';
import ActivityPanel from './ActivityPanel.svelte';
import MessageLine from './MessageLine.svelte';
import { activity, lane, row } from './fixtures';
import { actorView } from '@meldframe/web-document/fixtures';

const html = (a: ActivityState, extra: { checkpoints?: Checkpoint[]; self?: string; canCreate?: boolean; status?: 'unread' | 'loaded' | 'absent' } = {}) =>
  render(ActivityPanel as never, { props: { activity: a, checkpoints: [], canCreate: true, status: 'loaded', self: 'person:ana', dispatch: () => {}, ...extra } as never }).body.replace(/<!--.*?-->/g, '');
const cp = (id: string, seq: number): Checkpoint => ({ id, n: seq, seq, label: id, kind: 'manual', atMs: 1 });

describe('the Activity panel', () => {
  it('says what it is for when there is nothing yet', () => {
    const out = html(activity());
    expect(out).toContain('data-testid="activity-empty"');
    expect(out).not.toContain('data-testid="lane"');
  });

  it('draws a lane with its state, counts and rows, and opens only the most active lane', () => {
    const a = activity({
      lanes: [lane({ actor: 'agent:claude', state: 'working', ops: 12, batches: ['k2', 'k1'] }), lane({ actor: 'agent:cursor', batches: ['k9'] })],
      rows: [row({ n: 2, keyPrefix: 'k2' }), row({ n: 1, keyPrefix: 'k1' }), row({ actor: 'agent:cursor', n: 9, keyPrefix: 'k9' })],
    });
    const out = html(a);
    expect(out).toContain('Agent, working, 12 changes, 2 batches');
    expect(out.match(/data-testid="batch"/g)).toHaveLength(2);
    expect(out.indexOf('Batch 2')).toBeLessThan(out.indexOf('Batch 1'));
    expect(out).not.toContain('Batch 9');
    expect(out.match(/aria-expanded="true"/g)?.length).toBeGreaterThanOrEqual(1);
    expect(out).toMatch(/aria-controls="lane-body-1"[^>]*>|aria-expanded="false"[^>]*aria-controls="lane-body-1"/);
  });

  it('draws the kind of every lane beside its name, and the full actor as the title', () => {
    const a = activity({ lanes: [lane({ actor: 'agent:You', batches: [] }), lane({ actor: 'person:ana', batches: [] })] });
    const out = html(a, { self: 'person:ana' });
    expect(out).toMatch(/title="agent:You"[^>]*><bdi>You<\/bdi>/);
    expect(out).toMatch(/title="person:ana"[^>]*><bdi>You<\/bdi>/);
    expect(out).toContain('Agent, idle, 3 changes');
    expect(out).toContain('Person, idle, 3 changes');
  });

  it('counts the batches of a lane as the app layer lists them, even when the page holds fewer rows', () => {
    const a = activity({ lanes: [lane({ ops: 9, batches: ['k3', 'k2', 'k1'] })], rows: [row({ keyPrefix: 'k1' })] });
    expect(html(a)).toContain('Agent, idle, 9 changes, 3 batches');
  });

  it('shows the runs of a lane that has no batch, as the lane of a person does', () => {
    const a = activity({
      lanes: [lane({ actor: 'person:ana', ops: 12, batches: [], runs: [{ startMs: new Date(2026, 10, 20, 10, 41, 0).getTime(), endMs: new Date(2026, 10, 20, 10, 44, 5).getTime(), ops: 12 }] })],
    });
    const out = html(a, { self: 'person:ana' });
    expect(out).toContain('data-testid="lane-runs"');
    expect(out).toContain('10:41:00 to 10:44:05: 12 changes');
    expect(out).toContain('>You<');
  });

  it('draws what a batch did and how far it got', () => {
    const a = activity({
      lanes: [lane({ batches: ['k1'] })],
      rows: [row({ n: 3, status: 'partial', written: 18, commands: 30, atMs: new Date(2026, 10, 20, 10, 42, 31).getTime(), summary: { added: 2, changed: 1, removed: 0, names: ['Hero'] } })],
    });
    const out = html(a);
    expect(out).toContain('Batch 3');
    expect(out).toContain('18 of 30 applied');
    expect(out).toContain('10:42:31');
    expect(out).toContain('2 added, 1 changed: Hero');
  });

  it('marks the selected batch', () => {
    const a = activity({ lanes: [lane({ batches: ['k1'] })], rows: [row()], selected: { actor: 'agent:claude', keyPrefix: 'k1' } });
    expect(html(a)).toMatch(/data-testid="batch"[^>]*aria-current="true"/);
    expect(html({ ...a, selected: null })).not.toContain('aria-current="true"');
  });
});

describe('the note of a batch is text an agent wrote', () => {
  const withNote = (note: string) => html(activity({ lanes: [lane({ batches: ['k1'] })], rows: [row({ note })] }));

  it('shows it quoted, attributed to its actor and with its line breaks kept', () => {
    const out = withNote('Tighten the hero.\nBind the brand colours.');
    expect(out).toContain('Agent note from <bdi>agent:claude</bdi>');
    expect(out).toMatch(/<blockquote[^>]*whitespace-pre-line[^>]*>“<bdi>Tighten the hero\.\nBind the brand colours\.<\/bdi>”<\/blockquote>/);
  });

  it('writes markup as the text it is: no element, no link, no script', () => {
    const out = withNote('<script>alert(1)</script><a href="https://x.test">go</a> <img src=x onerror=alert(1)>');
    const note = out.slice(out.indexOf('data-testid="batch-note"'), out.indexOf('</figure>'));
    expect(note).toContain('&lt;script>alert(1)&lt;/script>');
    expect(note).toContain('&lt;a href="https://x.test">go&lt;/a>');
    expect(note).not.toMatch(/<(script|a|img)[\s>]/);
  });

  it('keeps a bidirectional override inside an isolate, so it cannot reorder the words around it', () => {
    const out = withNote('safe ‮txt.exe');
    expect(out).toContain('“<bdi>safe ‮txt.exe</bdi>”');
  });

  it('does not make a note out of nothing, and does not let a note pose as a line of the panel', () => {
    const none = html(activity({ lanes: [lane({ batches: ['k1'] })], rows: [row({ note: null })] }));
    expect(none).not.toContain('data-testid="batch-note"');
    const out = withNote('Applied. Nothing to revert.');
    expect(out).toContain('data-testid="batch-status">Applied<');
    expect(out).toContain('Agent note from');
  });

  it('draws a note whole, whatever its length: there is no fold that could hide text without a way to open it', () => {
    const cases: Record<string, string> = {
      short: 'Tidy the hero.',
      'about 200 characters': 'word '.repeat(40).trim(),
      'many line breaks': 'one\ntwo\nthree\nfour',
      'the longest note': `${'word '.repeat(99)}end`.slice(0, 500),
    };
    for (const [name, note] of Object.entries(cases)) {
      const out = withNote(note);
      const quoted = out.slice(out.indexOf('<blockquote'), out.indexOf('</blockquote>'));
      expect(quoted, name).toContain(note);
      expect(out, name).not.toContain('line-clamp');
      expect(out, name).not.toContain('Show more');
      expect(out, name).not.toContain('Show less');
    }
  });

  it('breaks a word with no spaces instead of running out of the card', () => {
    expect(withNote('x'.repeat(500))).toMatch(/<blockquote[^>]*break-words/);
  });
});

describe('Show and Revert', () => {
  const one = (over = {}) => html(activity({ lanes: [lane({ batches: ['k1'] })], rows: [row(over)] }));
  it('offers Show and Revert on a batch whose ops the page holds', () => {
    const out = one();
    expect(out).toContain('data-testid="batch-show"');
    expect(out).toContain('data-testid="batch-revert"');
    expect(out).not.toContain('data-testid="revert-confirm"');
  });
  it('says a batch that cannot be reverted from this page cannot, without guessing why, instead of offering the button', () => {
    const out = one({ revertable: false, canShow: false });
    expect(out).not.toContain('data-testid="batch-revert"');
    expect(out).toContain('data-testid="batch-not-revertable"');
    expect(out).toContain('Cannot be reverted from this page.');
    expect(out).not.toContain('Older than this page');
    expect(out).toMatch(/data-testid="batch-show"[^>]*disabled|disabled[^>]*data-testid="batch-show"/);
  });
  it('offers neither the button nor the line about it for a row with no number', () => {
    const out = one({ n: null, status: 'headerless', revertable: false });
    expect(out).not.toContain('data-testid="batch-revert"');
    expect(out).not.toContain('data-testid="batch-not-revertable"');
  });
});

describe('messages, checkpoints and older batches', () => {
  it('keeps a live region on the page before there is anything to say, and puts the message into it', () => {
    expect(html(activity())).toMatch(/<div role="status" data-testid="activity-message"><\/div>/);
    const line = render(MessageLine as never, { props: { message: { tone: 'warning', text: 'You may not accept or flag batches in this file.' } } as never }).body.replace(/<!--.*?-->/g, '');
    expect(line).toMatch(/<div role="status" data-testid="activity-message"><p data-tone="warning"[^>]*>You may not accept/);
    // events that were there when the panel opened are not said again
    const withOld = render(ActivityPanel as never, { props: { activity: activity(), checkpoints: [], self: 'x', dispatch: () => {}, events: [{ type: 'refused', code: 'forbidden' }] } as never }).body.replace(/<!--.*?-->/g, '');
    expect(withOld).toMatch(/<div role="status" data-testid="activity-message"><\/div>/);
  });
  it('lists the checkpoints newest first, each with Restore', () => {
    const out = html(activity(), { checkpoints: [cp('Beta', 20), cp('Alpha', 10)] });
    expect(out.indexOf('Beta')).toBeLessThan(out.indexOf('Alpha'));
    expect(out).toContain('After change 20');
    expect(out.match(/data-testid="checkpoint-restore"/g)).toHaveLength(2);
    expect(html(activity())).toContain('No checkpoint yet');
  });
  it('offers older batches only when the app layer says there are some', () => {
    expect(html(activity({ hasOlder: true }))).toContain('data-testid="older"');
    expect(html(activity({ hasOlder: false }))).not.toContain('data-testid="older"');
  });
});

describe('the review of a batch', () => {
  const one = (over = {}, a: Partial<ActivityState> = {}) => html(activity({ lanes: [lane({ batches: ['k1'] })], rows: [row(over)], ...a }));

  it('shows the state of every batch to everyone, in words, and the controls to nobody who may not review', () => {
    for (const [review, words] of [['unreviewed', 'Not reviewed'], ['accepted', 'Accepted'], ['needs_review', 'Needs review']] as const) {
      const out = one({ review }, { canReview: false });
      expect(out).toContain(`data-testid="batch-review">${words}<`);
      expect(out).not.toContain('data-testid="review-set"');
    }
  });

  it('offers Accept and Needs review to a person who may review, each named for its batch', () => {
    const out = one({ n: 3 }, { canReview: true });
    expect(out.match(/data-testid="review-set"/g)).toHaveLength(2);
    expect(out).toContain('aria-label="Accept: Batch 3"');
    expect(out).toContain('aria-label="Needs review: Batch 3"');
  });

  it('offers Clear on a reviewed batch, and no control on a batch with no number', () => {
    expect(one({ review: 'accepted' }, { canReview: true })).toContain('aria-label="Clear: Batch 1"');
    const out = one({ n: null, status: 'headerless', revertable: false }, { canReview: true });
    expect(out).not.toContain('data-testid="review-set"');
    expect(out).not.toContain('data-testid="batch-review"');
  });

  it('marks a batch that needs review with the review tint and the word, and an accepted one with neither', () => {
    expect(one({ review: 'needs_review' })).toMatch(/data-testid="batch-review"[^>]*bg-review-bg[^>]*text-review-text|bg-review-bg[^>]*text-review-text[^>]*data-testid="batch-review"/);
    expect(one({ review: 'accepted' })).not.toContain('bg-review-bg');
  });

  it('says who last changed the state', () => {
    const out = one({ review: 'accepted', reviewedBy: 'person:ana', reviewedAtMs: new Date(2026, 10, 20, 10, 50, 2).getTime() });
    expect(out).toContain('Last changed by <bdi>person:ana</bdi> at 10:50:02');
    expect(out).not.toMatch(/Unreviewed by/i);
  });

  it('keeps person:claude and agent:claude apart in the caption of the person who last changed a state', () => {
    expect(one({ review: 'accepted', reviewedBy: 'agent:claude' })).toContain('Last changed by <bdi>agent:claude</bdi>');
    expect(one({ review: 'accepted', reviewedBy: 'person:claude' })).toContain('Last changed by <bdi>person:claude</bdi>');
  });

  it('keeps the review state of a batch that is no longer in effect, and says both', () => {
    const out = one({ review: 'needs_review', inEffect: false });
    expect(out).toContain('Needs review');
    expect(out).toContain('data-testid="batch-not-in-effect"');
    expect(one({ inEffect: true })).not.toContain('data-testid="batch-not-in-effect"');
    expect(one({ inEffect: null })).not.toContain('data-testid="batch-not-in-effect"');
  });

  it('counts the batches to review at the top, from the server count and not from the rows it holds', () => {
    const out = one({}, { unreviewedCount: 12 });
    expect(out).toContain('data-testid="review-count">12 batches to review<');
    expect(one({}, { unreviewedCount: 1 })).toContain('1 batch to review');
    expect(one({}, { unreviewedCount: 0 })).not.toContain('data-testid="review-count"');
  });

  it('says Unknown when the count is unknown (the list did not load), and neither zero nor nothing waiting', () => {
    const out = one({}, { unreviewedCount: null });
    expect(out).toMatch(/data-testid="review-count"[^>]*data-unknown="true"[^>]*>Batches to review: Unknown/);
    expect(out).not.toContain('0 batches');
    expect(out).not.toContain('Nothing waiting');
  });
});

describe('Create checkpoint', () => {
  it('has a text field with a visible label, at most 128 characters, and a button', () => {
    const out = html(activity());
    expect(out).toMatch(/<label[^>]*for="([^"]+)"[^>]*>Name \(optional\)<\/label>/);
    const id = /<label[^>]*for="([^"]+)"/.exec(out)![1]!;
    expect(out).toMatch(new RegExp(`<input[^>]*id="${id}"[^>]*maxlength="128"|<input[^>]*maxlength="128"[^>]*id="${id}"`));
    expect(out).toMatch(/data-testid="checkpoint-create"[^>]*>Create checkpoint</);
    expect(out).not.toContain('Save checkpoint');
  });

  it('is enabled for a caller who may make one', () => {
    expect(html(activity(), { canCreate: true })).not.toMatch(/data-testid="checkpoint-create"[^>]*\sdisabled=""/);
    expect(html(activity(), { canCreate: true })).not.toContain('data-testid="checkpoint-create-why"');
  });

  it('is disabled where the caller may not, and says why in text that describes the button and the field', () => {
    const out = html(activity(), { canCreate: false });
    expect(out).toMatch(/<button[^>]*data-testid="checkpoint-create"[^>]*\sdisabled=""|<button[^>]*\sdisabled=""[^>]*data-testid="checkpoint-create"/);
    expect(out).toMatch(/<input[^>]*data-testid="checkpoint-name"[^>]*\sdisabled=""|<input[^>]*\sdisabled=""[^>]*data-testid="checkpoint-name"/);
    const why = /id="([^"]+)"[^>]*data-testid="checkpoint-create-why"|data-testid="checkpoint-create-why"[^>]*id="([^"]+)"/.exec(out)!;
    expect(out).toContain(`aria-describedby="${why[1] ?? why[2]}"`);
    expect(out.match(/aria-describedby="/g)).toHaveLength(2);
  });

  it('words the reason for each case: not loaded yet, no module on the server, or an account that may not', () => {
    const say = (status: 'unread' | 'loaded' | 'absent') => /data-testid="checkpoint-create-why"[^>]*>([^<]*)</.exec(html(activity(), { canCreate: false, status }))![1];
    expect(say('unread')).toBe('The checkpoints have not loaded yet.');
    expect(say('absent')).toBe('This server does not keep checkpoints.');
    expect(say('loaded')).toBe('This account may not make checkpoints in this file.');
  });

  it('shows a label as text: a script in it is escaped, a long one is clipped by the style and kept in the title', () => {
    const evil = '<script>alert(1)</script>';
    const long = 'L'.repeat(128);
    const out = html(activity(), { checkpoints: [{ ...cp('2', 2), label: evil }, { ...cp('1', 1), label: long }] });
    expect(out).not.toContain('<script>alert');
    expect(out).toContain('&lt;script>alert(1)&lt;/script>');
    expect(out).toMatch(/line-clamp-2[^>]*title="L{128}"|title="L{128}"[^>]*line-clamp-2/);
  });

  it('names who made a manual checkpoint, and nobody for the ones the server took itself', () => {
    const run: Checkpoint = { id: '3', n: 3, seq: 5, kind: 'runStart', about: actorView('agent:claude'), atMs: 1 };
    const out = html(activity(), { checkpoints: [run, { ...cp('2', 2), by: actorView('person:bea') }] });
    expect(out).toContain('Made by bea');
    expect(out).toContain("Before claude's run");
    expect(out.match(/Made by/g)).toHaveLength(1);
  });
});
