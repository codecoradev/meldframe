// The React copy of the Activity panel, second part (the rows of a batch, the review controls, the question of a revert or a restore,
// Create checkpoint and the list of checkpoints), checked as panel.test.ts checks the Svelte one: the markup of a server render, in
// Node, and the same render of the two copies side by side. Clicks, the keyboard focus that a question takes and gives back, Enter in
// the name field and the field that empties when a checkpoint is made need a browser (activity-e2e.mjs and checkpoints-e2e.mjs with
// MELDFRAME_UI=react; pixels-e2e.mjs for the pixels).
import type { ActivityState, Checkpoint, Confirm } from '@meldframe/protocol';
import { actorView } from '@meldframe/web-document/fixtures';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import ActivityPanelSvelte from '../ActivityPanel.svelte';
import { activity, lane, row } from '../fixtures';
import { ActivityPanel } from './ActivityPanel';

type Extra = { checkpoints?: Checkpoint[]; canCreate?: boolean; status?: 'unread' | 'loaded' | 'absent'; confirm?: Confirm | null; self?: string };
const props = (a: ActivityState, over: Extra = {}) => ({ activity: a, checkpoints: [], canCreate: true, status: 'loaded' as const, self: 'person:ana', dispatch: () => {}, ...over });
const html = (a: ActivityState, over: Extra = {}) => renderToStaticMarkup(<ActivityPanel {...props(a, over)} />);
const svelte = (a: ActivityState, over: Extra = {}) => render(ActivityPanelSvelte as never, { props: props(a, over) as never }).body.replace(/<!--.*?-->/g, '');
const cp = (id: string, seq: number, over: Partial<Checkpoint> = {}): Checkpoint => ({ id, n: seq, seq, label: id, kind: 'manual', atMs: 1, ...over });
const one = (over = {}, a: Partial<ActivityState> = {}, extra: Extra = {}) => html(activity({ lanes: [lane({ batches: ['k1'] })], rows: [row(over)], ...a }), extra);

describe('the rows of a batch', () => {
  it('draws a lane with its batches newest first, a count from the app layer and the batches the page holds', () => {
    const a = activity({
      lanes: [lane({ actor: 'agent:claude', state: 'working', ops: 12, batches: ['k2', 'k1'] }), lane({ actor: 'agent:cursor', batches: ['k9'] })],
      rows: [row({ n: 2, keyPrefix: 'k2' }), row({ n: 1, keyPrefix: 'k1' }), row({ actor: 'agent:cursor', n: 9, keyPrefix: 'k9' })],
    });
    const out = html(a);
    expect(out).toContain('Agent, working, 12 changes, 2 batches');
    expect(out.match(/data-testid="batch"/g)).toHaveLength(2);
    expect(out.indexOf('Batch 2')).toBeLessThan(out.indexOf('Batch 1'));
    expect(out).not.toContain('Batch 9'); // the second lane is folded
    expect(html(activity({ lanes: [lane({ ops: 9, batches: ['k3', 'k2', 'k1'] })], rows: [row({ keyPrefix: 'k1' })] }))).toContain('Agent, idle, 9 changes, 3 batches');
  });
  it('draws what a batch did, how far it got and when, and marks the selected one', () => {
    const out = one({ n: 3, status: 'partial', written: 18, commands: 30, atMs: new Date(2026, 10, 20, 10, 42, 31).getTime(), summary: { added: 2, changed: 1, removed: 0, names: ['Hero'] } });
    expect(out).toContain('>Batch 3<');
    expect(out).toContain('>18 of 30 applied<');
    expect(out).toContain('>10:42:31<');
    expect(out).toContain('>2 added, 1 changed: Hero<');
    const sel = activity({ lanes: [lane({ batches: ['k1'] })], rows: [row()], selected: { actor: 'agent:claude', keyPrefix: 'k1' } });
    expect(html(sel)).toMatch(/data-testid="batch"[^>]*aria-current="true"/);
    expect(html({ ...sel, selected: null })).not.toContain('aria-current="true"');
  });
  it('shows the note as text an agent wrote: quoted, attributed to its actor, whole, in an isolate, and never as markup', () => {
    const out = one({ note: 'Tighten the hero.\nBind the brand colours.' });
    expect(out).toContain('Agent note from <bdi>agent:claude</bdi>');
    expect(out).toMatch(/<blockquote[^>]*whitespace-pre-line[^>]*>“<bdi>Tighten the hero\.\nBind the brand colours\.<\/bdi>”<\/blockquote>/);
    const evil = one({ note: '<script>alert(1)</script><a href="https://x.test">go</a> <img src=x onerror=alert(1)>' });
    const note = evil.slice(evil.indexOf('data-testid="batch-note"'), evil.indexOf('</figure>'));
    expect(note).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(note).not.toMatch(/<(script|a|img)[\s>]/);
    expect(one({ note: 'safe \u202etxt.exe' })).toContain('“<bdi>safe \u202etxt.exe</bdi>”');
    expect(one({ note: null })).not.toContain('data-testid="batch-note"');
    expect(one({ note: 'x'.repeat(500) })).toMatch(/<blockquote[^>]*break-words/);
    expect(one({ note: 'word '.repeat(99) })).not.toContain('line-clamp');
  });
  it('offers Show and Revert on a batch whose ops the page holds, and says so, in words, on one that cannot be reverted', () => {
    const ok = one();
    expect(ok).toContain('data-testid="batch-show"');
    expect(ok).toContain('data-testid="batch-revert"');
    expect(ok).not.toContain('data-testid="revert-confirm"');
    const no = one({ revertable: false, canShow: false });
    expect(no).not.toContain('data-testid="batch-revert"');
    expect(no).toContain('>Cannot be reverted from this page.<');
    expect(no).toMatch(/<button[^>]*data-testid="batch-show"[^>]*\sdisabled=""|<button[^>]*\sdisabled=""[^>]*data-testid="batch-show"/);
    const bare = one({ n: null, status: 'headerless', revertable: false });
    expect(bare).not.toContain('data-testid="batch-revert"');
    expect(bare).not.toContain('data-testid="batch-not-revertable"');
    expect(bare).toContain('>Batch without a number<');
  });
  it('draws the question only on the row the app layer holds it for, with the wording of a batch still in effect or not', () => {
    const a = () => activity({ lanes: [lane({ batches: ['k1', 'k2'] })], rows: [row({ n: 1, keyPrefix: 'k1' }), row({ n: 2, keyPrefix: 'k2', inEffect: false })] });
    const ask = (keyPrefix: string) => html(a(), { confirm: { kind: 'revert', actor: 'agent:claude', keyPrefix } });
    expect(html(a())).not.toContain('revert-confirm');
    expect(ask('k1').match(/data-testid="revert-confirm"/g)).toHaveLength(1);
    expect(ask('k1')).toContain('aria-label="Confirm the revert of Batch 1"');
    expect(ask('k1')).toContain('>Revert Batch 1? What its agent wrote last goes back, as a new batch of yours. What others wrote since stays.<');
    expect(ask('k1')).toMatch(/data-testid="revert-confirm-yes"[^>]*>Revert</);
    expect(ask('k1')).toMatch(/data-testid="revert-cancel"[^>]*>Cancel</);
    expect(ask('k2')).toContain('Nothing left to revert');
    expect(ask('k2')).not.toContain('revert-confirm-yes');
    expect(ask('k2')).toMatch(/data-testid="revert-cancel"[^>]*>Close</);
    expect(ask('k9')).not.toContain('revert-confirm');
  });
  it('shows a batch that is no longer in effect with its review state', () => {
    const out = one({ review: 'needs_review', inEffect: false });
    expect(out).toContain('>Needs review<');
    expect(out).toContain('data-testid="batch-not-in-effect"');
    expect(one({ inEffect: true })).not.toContain('data-testid="batch-not-in-effect"');
    expect(one({ inEffect: null })).not.toContain('data-testid="batch-not-in-effect"');
  });
});

describe('the review of a batch', () => {
  it('shows the state of every batch to everyone, in words, and the controls to nobody who may not review', () => {
    for (const [review, words] of [['unreviewed', 'Not reviewed'], ['accepted', 'Accepted'], ['needs_review', 'Needs review']] as const) {
      const out = one({ review }, { canReview: false });
      expect(out).toContain(`data-testid="batch-review">${words}<`);
      expect(out).not.toContain('data-testid="review-set"');
    }
  });
  it('offers Accept and Needs review to a person who may review, each named for its batch, and Clear on a reviewed one', () => {
    const out = one({ n: 3 }, { canReview: true });
    expect(out.match(/data-testid="review-set"/g)).toHaveLength(2);
    expect(out).toContain('aria-label="Accept: Batch 3"');
    expect(out).toContain('aria-label="Needs review: Batch 3"');
    expect(out).toContain('role="group" aria-label="Review Batch 3"');
    expect(one({ review: 'accepted' }, { canReview: true })).toContain('aria-label="Clear: Batch 1"');
    expect(one({ review: 'accepted' }, { canReview: true })).not.toContain('aria-label="Accept: Batch 1"');
    const bare = one({ n: null, status: 'headerless', revertable: false }, { canReview: true });
    expect(bare).not.toContain('data-testid="review-set"');
    expect(bare).not.toContain('data-testid="batch-review"');
  });
  it('marks a batch that needs review with the tint and the word, and says who last changed the state in an isolate', () => {
    expect(one({ review: 'needs_review' })).toMatch(/bg-review-bg text-review-text[^>]*data-testid="batch-review"/);
    expect(one({ review: 'accepted' })).not.toContain('bg-review-bg');
    const out = one({ review: 'accepted', reviewedBy: 'person:ana', reviewedAtMs: new Date(2026, 10, 20, 10, 50, 2).getTime() });
    expect(out).toContain('Last changed by <bdi>person:ana</bdi> at 10:50:02');
    expect(out).not.toMatch(/Unreviewed by/i);
    expect(one({ review: 'accepted', reviewedBy: 'person:claude' })).toContain('Last changed by <bdi>person:claude</bdi>');
  });
});

describe('Create checkpoint and the checkpoints', () => {
  it('has a text field with a visible label, at most 128 characters, and a button', () => {
    const out = html(activity());
    const id = /<label[^>]*for="([^"]+)"[^>]*>Name \(optional\)<\/label>/.exec(out)![1]!;
    expect(out).toMatch(new RegExp(`<input[^>]*id="${id.replace(/[^\w-]/g, '\\$&')}"[^>]*maxLength="128"|<input[^>]*maxlength="128"[^>]*id="${id.replace(/[^\w-]/g, '\\$&')}"`, 'i'));
    expect(out).toMatch(/data-testid="checkpoint-create"[^>]*>Create checkpoint</);
    expect(out).not.toMatch(/data-testid="checkpoint-create"[^>]*\sdisabled=""/);
    expect(out).not.toContain('data-testid="checkpoint-create-why"');
  });
  it('is disabled where the caller may not, and says why in text that describes the button and the field, for each case', () => {
    const say = (status: 'unread' | 'loaded' | 'absent') => /data-testid="checkpoint-create-why"[^>]*>([^<]*)</.exec(html(activity(), { canCreate: false, status }))![1];
    expect(say('unread')).toBe('The checkpoints have not loaded yet.');
    expect(say('absent')).toBe('This server does not keep checkpoints.');
    expect(say('loaded')).toBe('This account may not make checkpoints in this file.');
    const out = html(activity(), { canCreate: false });
    expect(out).toMatch(/<button[^>]*data-testid="checkpoint-create"[^>]*\sdisabled=""|<button[^>]*\sdisabled=""[^>]*data-testid="checkpoint-create"/);
    expect(out).toMatch(/<input[^>]*data-testid="checkpoint-name"[^>]*\sdisabled=""|<input[^>]*\sdisabled=""[^>]*data-testid="checkpoint-name"/);
    const why = /id="([^"]+)"[^>]*data-testid="checkpoint-create-why"/.exec(out)![1]!;
    expect(out.match(new RegExp(`aria-describedby="${why.replace(/[^\w-]/g, '\\$&')}"`, 'g'))).toHaveLength(2);
  });
  it('lists the checkpoints newest first, each with Restore, and says so when there is none', () => {
    const out = html(activity(), { checkpoints: [cp('Beta', 20), cp('Alpha', 10)] });
    expect(out.indexOf('Beta')).toBeLessThan(out.indexOf('Alpha'));
    expect(out).toContain('>After change 20<');
    expect(out.match(/data-testid="checkpoint-restore"/g)).toHaveLength(2);
    expect(html(activity())).toContain('>No checkpoint yet. The server makes one when an agent&#x27;s run starts, and one goes first before a revert or a restore.<');
  });
  it('draws the question on the checkpoint the app layer holds it for, and on no other', () => {
    const out = html(activity(), { checkpoints: [cp('c1', 1), cp('c2', 2)], confirm: { kind: 'restore', id: 'c2' } });
    expect(out.match(/data-testid="restore-confirm"/g)).toHaveLength(1);
    expect(out.match(/data-testid="checkpoint-restore"/g)).toHaveLength(1);
    expect(out).toContain('aria-label="Confirm the restore of this checkpoint"');
    expect(out).toMatch(/data-testid="restore-yes"[^>]*>Restore</);
    expect(html(activity(), { checkpoints: [cp('c1', 1)] })).not.toContain('restore-confirm');
  });
  it('shows a label as text, clipped by the style and whole in the title, and names who made a manual checkpoint only', () => {
    const evil = '<script>alert(1)</script>';
    const out = html(activity(), { checkpoints: [cp('2', 2, { label: evil }), cp('1', 1, { label: 'L'.repeat(128) })] });
    expect(out).not.toContain('<script>alert');
    expect(out).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(out).toMatch(/line-clamp-2[^>]*title="L{128}"/);
    const run: Checkpoint = { id: '3', n: 3, seq: 5, kind: 'runStart', about: actorView('agent:claude'), atMs: 1 };
    const by = html(activity(), { checkpoints: [run, cp('2', 2, { by: actorView('person:bea') })] });
    expect(by).toContain('Made by bea');
    expect(by).toContain('Before claude&#x27;s run');
    expect(by.match(/Made by/g)).toHaveLength(1);
  });
});

// The two copies draw the same elements with the same classes in the same order, with the ids of the generated elements given a
// number in order of appearance (React and Svelte make them differently). What a class cannot say is the pixels: pixels-e2e.mjs.
describe('the React Activity panel against the Svelte one, whole', () => {
  const classes = (out: string) => [...out.matchAll(/class="([^"]*)"/g)].map((m) => m[1]!.split(/\s+/).filter(Boolean).sort().join(' '));
  const ID = ['id', 'for', 'aria-describedby', 'aria-controls', 'aria-labelledby'];
  /** The attribute values by name, with every id turned into its place in the list of ids, so that two copies' ids compare. */
  const attrs = (out: string, name: string) => {
    const ids = [...out.matchAll(/\sid="([^"]*)"/g)].map((m) => m[1]!);
    return [...out.matchAll(new RegExp(`\\s${name}="([^"]*)"`, 'g'))].map((m) => (ID.includes(name) ? `#${ids.indexOf(m[1]!)}` : m[1]!));
  };
  const NAMES = ['data-testid', 'data-actor', 'data-state', 'data-tone', 'data-n', 'data-status', 'data-checkpoint', 'data-unknown', 'data-state', 'role', 'aria-label', 'aria-expanded', 'aria-current', 'title', 'type', 'maxlength', 'disabled', 'autocomplete', 'value', ...ID];
  const text = (out: string) => out.split(/<[^>]*>/).filter((t) => t.trim() !== '');
  // The two serializers write the apostrophe and the angle bracket in a text or an attribute differently (`&#x27;` and `'`, `&gt;` and `>`),
  // and React keeps the camel case of two attribute names that HTML reads without it.
  const entities = (o: string) => o.replaceAll('&#x27;', "'").replaceAll('&#39;', "'").replaceAll('&gt;', '>');
  const same = (a: ActivityState, over: Extra = {}) => {
    const [r, s] = [entities(html(a, over).replace(/maxLength=/g, 'maxlength=').replace(/autoComplete=/g, 'autocomplete=')), entities(svelte(a, over))];
    expect(classes(r)).toEqual(classes(s));
    for (const name of NAMES) expect(attrs(r, name), name).toEqual(attrs(s, name));
    expect(text(r)).toEqual(text(s));
    expect((r.match(/<[a-z0-9]+/g) ?? []).join()).toEqual((s.match(/<[a-z0-9]+/g) ?? []).join());
  };
  const rows = [
    row({ n: 3, keyPrefix: 'k3', note: 'Tighten the hero.\n<b>not bold</b> “quoted”', summary: { added: 2, changed: 1, removed: 0, names: ['Hero', 'Title'] }, atMs: new Date(2026, 10, 20, 10, 42, 31).getTime() }),
    row({ n: 2, keyPrefix: 'k2', status: 'partial', written: 18, commands: 30, review: 'needs_review', reviewedBy: 'person:ana', reviewedAtMs: new Date(2026, 10, 20, 10, 50, 2).getTime(), inEffect: false }),
    row({ n: 1, keyPrefix: 'k1', review: 'accepted', revertable: false, canShow: false, summary: null }),
    row({ n: null, keyPrefix: 'k0', status: 'headerless', revertable: false, summary: null }),
  ];
  const base = () => activity({ lanes: [lane({ actor: 'agent:claude', state: 'working', ops: 12, batches: ['k3', 'k2', 'k1', 'k0'] }), lane({ actor: 'person:ana', batches: [] })], rows, unreviewedCount: 2, canReview: true, selected: { actor: 'agent:claude', keyPrefix: 'k2' }, hasOlder: true });
  it('has the same markup for lanes with batches of every status and review state, for a reviewer and for a reader', () => {
    same(base());
    same({ ...base(), canReview: false });
  });
  it('has the same markup with the question of a revert open on a batch in effect, on one that is not, and on none', () => {
    same(base(), { confirm: { kind: 'revert', actor: 'agent:claude', keyPrefix: 'k3' } });
    same(base(), { confirm: { kind: 'revert', actor: 'agent:claude', keyPrefix: 'k2' } });
    same(base(), { confirm: { kind: 'revert', actor: 'agent:claude', keyPrefix: 'k9' } });
  });
  it('has the same markup for the checkpoints: none, a list with the question open on one, labels, and each state of Create checkpoint', () => {
    const list = [cp('c3', 30, { label: '<script>x</script>' }), cp('c2', 20, { kind: 'runStart', label: undefined, about: actorView('agent:claude') }), cp('c1', 10, { by: actorView('person:bea'), label: 'L'.repeat(128) }), cp('c0', 5, { kind: 'beforeRevert', label: undefined, about: actorView('agent:claude') }), cp('cx', 4, { kind: 'beforeRestore', label: undefined })];
    same(activity());
    same(activity(), { checkpoints: list });
    same(activity(), { checkpoints: list, confirm: { kind: 'restore', id: 'c1' } });
    same(activity(), { canCreate: false, status: 'unread' });
    same(activity(), { canCreate: false, status: 'absent' });
    same(activity(), { canCreate: false, status: 'loaded' });
  });
});
