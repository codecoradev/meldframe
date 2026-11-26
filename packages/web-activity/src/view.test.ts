import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { Checkpoint, Event } from '@meldframe/protocol';
import { activity, lane, row } from './fixtures';
import { batchHeading, checkpointBy, checkpointMade, checkpointWords, clockText, createWhyNot, laneSummary, laneViews, messageFor, reviewChoices, reviewWords, reviewedBy, statusWords, summaryWords } from './view';
import { actorView } from '@meldframe/web-document/fixtures';

describe('laneViews', () => {
  it('lists the rows of a lane in the order the lane gives, and leaves out a key prefix that has no row', () => {
    const a = activity({
      lanes: [lane({ batches: ['k3', 'gone', 'k1'] })],
      rows: [row({ keyPrefix: 'k1', n: 1 }), row({ keyPrefix: 'k3', n: 3 })],
    });
    expect(laneViews(a, 'person:ana')[0]!.rows.map((r) => r.keyPrefix)).toEqual(['k3', 'k1']);
  });

  it('keeps a batch with the actor that wrote it when two actors use the same key prefix', () => {
    const a = activity({
      lanes: [lane({ actor: 'agent:a', batches: ['k1'] }), lane({ actor: 'agent:b', batches: ['k1'] })],
      rows: [row({ actor: 'agent:a', n: 1 }), row({ actor: 'agent:b', n: 2 })],
    });
    const v = laneViews(a, 'person:ana');
    expect(v.map((l) => l.rows.map((r) => r.n))).toEqual([[1], [2]]);
  });

  it('names a lane by its actor, and the page of the person as You', () => {
    const a = activity({ lanes: [lane({ actor: 'agent:claude' }), lane({ actor: 'person:ana', batches: [] })] });
    const v = laneViews(a, 'person:ana');
    expect(v.map((l) => l.name)).toEqual(['claude', 'You']);
    expect(v.map((l) => l.self)).toEqual([false, true]);
  });

  it('carries the tone the app layer gave the actor, the same as its layer rows and its rings, and works none out', () => {
    const a = activity({ lanes: [lane({ actor: 'agent:claude', view: actorView('agent:claude', 'agent3') }), lane({ actor: 'person:ana', batches: [] }), lane({ actor: 'agent:\u{1F300}', view: actorView('agent:\u{1F300}', 'agent2'), batches: [] })] });
    expect(laneViews(a, 'x').map((l) => l.tone)).toEqual(['agent3', 'person', 'agent2']);
    expect(laneViews(a, 'x').map((l) => l.kind)).toEqual(['agent', 'person', 'agent']);
  });

  it('names a lane from the view of the app layer, not from the actor string', () => {
    const a = activity({ lanes: [lane({ actor: 'agent:claude', view: actorView('agent:claude', 'agent1', 'Claude Code') })] });
    expect(laneViews(a, 'x')[0]!.name).toBe('Claude Code');
  });

  it('counts a lane\'s batches as the app layer lists them, not by the rows that could be joined', () => {
    const a = activity({ lanes: [lane({ batches: ['k3', 'gone', 'k1'] })], rows: [row({ keyPrefix: 'k1' })] });
    expect(laneViews(a, 'x')[0]!.batches).toBe(3);
    expect(laneViews(a, 'x')[0]!.rows).toHaveLength(1);
  });

  it('draws the kind beside the name, so an agent called You is not the person and person:claude is not agent:claude', () => {
    const a = activity({
      lanes: [lane({ actor: 'agent:You', batches: [] }), lane({ actor: 'person:ana', batches: [] }), lane({ actor: 'person:claude', batches: [] }), lane({ actor: 'agent:claude', batches: [] })],
    });
    const v = laneViews(a, 'person:ana');
    expect(v.map((l) => `${l.name}/${l.kindWords}`)).toEqual(['You/Agent', 'You/Person', 'claude/Person', 'claude/Agent']);
    expect(v.map((l) => l.actor)).toEqual(['agent:You', 'person:ana', 'person:claude', 'agent:claude']);
  });

  it('words a lane under its name: kind, state and counts, with the singular for one', () => {
    const l = (over: Partial<Parameters<typeof laneSummary>[0]>) => laneSummary({ kindWords: 'Agent', stateWords: 'Working', ops: 12, batches: 3, ...over });
    expect(l({})).toBe('Agent, working, 12 changes, 3 batches');
    expect(l({ ops: 1, batches: 1 })).toBe('Agent, working, 1 change, 1 batch');
    expect(l({ batches: 0, kindWords: 'Person' })).toBe('Person, working, 12 changes');
  });

  it('words the state of a lane', () => {
    const states = (['working', 'quiet', 'idle'] as const).map((state) => laneViews(activity({ lanes: [lane({ state })] }), 'x')[0]!.stateWords);
    expect(states).toEqual(['Working', 'Quiet', 'Idle']);
  });
});

describe('the words of a row', () => {
  it('heads a batch with its number, and a batch with no header without one', () => {
    expect(batchHeading(row({ n: 3 }))).toBe('Batch 3');
    expect(batchHeading(row({ n: null, status: 'headerless' }))).toBe('Batch without a number');
  });

  it('says how far a batch got: applied, partial with the counts, still arriving, or without a header', () => {
    expect(statusWords(row({ status: 'applied' }))).toBe('Applied');
    expect(statusWords(row({ status: 'partial', written: 18, commands: 30 }))).toBe('18 of 30 applied');
    // A row made from ops alone does not know how many commands the call held: it is not worded as a total.
    expect(statusWords(row({ status: 'partial', written: 18, commands: null }))).toBe('18 applied');
    expect(statusWords(row({ status: 'building' }))).toBe('Arriving');
    expect(statusWords(row({ status: 'headerless' }))).toBe('No header: no number, note or review');
  });

  it('counts what a batch did, leaves out the zeros and adds the names', () => {
    expect(summaryWords({ added: 2, changed: 1, removed: 0, names: ['Hero', 'Title'] })).toBe('2 added, 1 changed: Hero, Title');
    expect(summaryWords({ added: 0, changed: 0, removed: 1, names: [] })).toBe('1 removed');
    expect(summaryWords({ added: 0, changed: 0, removed: 0, names: [] })).toBe('No node changed');
    expect(summaryWords(null)).toBeNull();
  });

  it('writes the time of a batch in the local zone', () => {
    expect(clockText(new Date(2026, 10, 20, 10, 42, 31).getTime())).toBe('10:42:31');
    expect(clockText(new Date(2026, 10, 20, 9, 5, 7).getTime())).toBe('09:05:07');
  });
});

describe('checkpointWords', () => {
  const cp = (over: Partial<Checkpoint>): Checkpoint => ({ id: '1', n: 1, seq: 42, kind: 'manual', atMs: 1, ...over });
  it('uses the label the person typed, and words the checkpoints the server took by itself', () => {
    expect(checkpointWords(cp({ label: 'Before the pitch' }))).toBe('Before the pitch');
    expect(checkpointWords(cp({}))).toBe('Checkpoint');
    expect(checkpointWords(cp({ kind: 'runStart', about: actorView('agent:claude') }))).toBe("Before claude's run");
    expect(checkpointWords(cp({ kind: 'beforeRevert', about: actorView('agent:claude') }))).toBe("Before reverting claude's batch");
    expect(checkpointWords(cp({ kind: 'beforeRestore' }))).toBe('Before a restore');
  });
});

describe('messageFor', () => {
  const refused = (code: string): Event => ({ type: 'refused', code });
  it('words the refusals of the review and the batch routes', () => {
    expect(messageFor([refused('forbidden')])).toEqual({ tone: 'warning', text: 'You may not accept or flag batches in this file.' });
    expect(messageFor([refused('unknown_batch')])?.text).toBe('That batch is older than this page: it cannot be shown or reverted here.');
    expect(messageFor([refused('no_batch_number')])?.text).toBe('A batch with no number cannot be reviewed.');
    expect(messageFor([refused('unavailable')])?.text).toBe('The server could not be reached. Try again in a moment.');
  });

  // The codes come from the Rust sources, not from a copy here: a refusal the app layer can make for a revert, a restore,
  // Show or a review, and the codes of the answer of the batch routes.
  const rust = (path: string) => readFileSync(new URL(`../../../crates/${path}`, import.meta.url), 'utf8');
  const appCodes = (): string[] => {
    const found = new Set<string>();
    for (const src of [rust('app/meldframe-app/src/review.rs'), rust('app/meldframe-app/src/activity.rs'), rust('app/meldframe-app/src/checkpoints.rs')]) {
      for (const m of src.matchAll(/(?:refuse\(\s*&mut self\.events,|self\.say\()\s*"([a-z_]+)"\s*\)/g)) found.add(m[1]!);
    }
    // The plans of a revert and of a restore live in meldframe-app-rules and answer with the code as `Err("code")`.
    for (const m of rust('core/meldframe-app-rules/src/review.rs').matchAll(/Err\("([a-z_]+)"\)/g)) found.add(m[1]!);
    for (const m of rust('core/meldframe-activity/src/feed.rs').matchAll(/ok_or\("([a-z_]+)"\)/g)) found.add(m[1]!);
    return [...found].sort();
  };
  it('reads the codes from the sources, and finds the ones the review flows refuse with', () => {
    expect(appCodes()).toEqual(expect.arrayContaining(['batch_incomplete', 'checkpoint_failed', 'checkpoint_pending', 'log_mismatch', 'no_batch_number', 'nothing_to_restore', 'nothing_to_revert', 'unknown_batch', 'unknown_checkpoint']));
  });
  it('has words for every code the app layer refuses with, and never prints the code', () => {
    for (const code of [...appCodes(), 'forbidden', 'not_found', 'invalid', 'rate_limited', 'unavailable']) {
      const text = messageFor([refused(code)])?.text;
      expect(text, code).toBeTruthy();
      expect(text, code).not.toContain(code);
      expect(text, code).not.toMatch(/_/);
    }
  });

  it('leaves a code it has no words for to the toast of the page, and does not read a word off the prototype', () => {
    expect(messageFor([refused('odd_code')])).toBeNull();
    expect(messageFor([refused('constructor')])).toBeNull();
    expect(messageFor([refused('odd_code'), { type: 'reverted', requested: 1, skipped: [] }])?.text).toMatch(/^Reverted/);
  });

  it('says what a revert did and what it left alone', () => {
    expect(messageFor([{ type: 'reverted', requested: 4, skipped: [] }])).toEqual({ tone: 'info', text: 'Reverted: 4 changes sent. One undo takes them back.' });
    const skipped = [{ node: 'a', what: 'fill', reason: 'changedSince' as const }, { node: 'b', what: 'place', reason: 'notApplicable' as const }];
    expect(messageFor([{ type: 'reverted', requested: 1, skipped }])?.text).toBe('Reverted: 1 change sent, 2 parts left as they are. One undo takes them back.');
    expect(messageFor([{ type: 'restored', id: 'c1', requested: 2 }])?.text).toBe('Restored: 2 changes sent. One undo takes them back.');
  });

  it('takes the last message of the events, and nothing from the others', () => {
    expect(messageFor([refused('forbidden'), { type: 'reverted', requested: 1, skipped: [] }])?.text).toMatch(/^Reverted/);
    expect(messageFor([{ type: 'selectionChanged', ids: [] }, { type: 'activityChanged' }])).toBeNull();
  });
});

describe('the review state of a row', () => {
  it('words the three states', () => {
    expect(reviewWords('unreviewed')).toBe('Not reviewed');
    expect(reviewWords('accepted')).toBe('Accepted');
    expect(reviewWords('needs_review')).toBe('Needs review');
  });

  it('says who last changed it, as the full actor, and when', () => {
    const at = new Date(2026, 10, 20, 10, 50, 2).getTime();
    expect(reviewedBy(row({ review: 'accepted', reviewedBy: 'person:ana', reviewedAtMs: at }))).toEqual({ by: 'person:ana', at: '10:50:02' });
    // a state cleared back to unreviewed still names the person who cleared it
    expect(reviewedBy(row({ review: 'unreviewed', reviewedBy: 'person:ana', reviewedAtMs: at }))).toEqual({ by: 'person:ana', at: '10:50:02' });
    expect(reviewedBy(row({ review: 'accepted', reviewedBy: 'person:ana', reviewedAtMs: null }))).toEqual({ by: 'person:ana', at: null });
    expect(reviewedBy(row({ reviewedBy: null }))).toBeNull();
  });

  it('offers the other states, and only where the app layer says this person may review', () => {
    const labels = (r: ReturnType<typeof row>, can: boolean) => reviewChoices(r, can).map((c) => `${c.state}:${c.label}`);
    expect(labels(row({ review: 'unreviewed' }), true)).toEqual(['accepted:Accept', 'needs_review:Needs review']);
    expect(labels(row({ review: 'accepted' }), true)).toEqual(['needs_review:Needs review', 'unreviewed:Clear']);
    expect(labels(row({ review: 'needs_review' }), true)).toEqual(['accepted:Accept', 'unreviewed:Clear']);
    expect(labels(row({ review: 'unreviewed' }), false)).toEqual([]);
  });

  it('offers nothing for a row with no number, which the server cannot review', () => {
    expect(reviewChoices(row({ n: null, status: 'headerless' }), true)).toEqual([]);
  });
});

describe('checkpoints in words', () => {
  const made = (over: Partial<Checkpoint>): Checkpoint => ({ id: '1', n: 1, seq: 4, kind: 'manual', atMs: 1, by: actorView('person:ana'), ...over });
  it('names the actor that made a manual one, and says nothing of who asked for the others', () => {
    expect(checkpointBy(made({ label: 'v1' }))).toBe('Made by ana');
    expect(checkpointBy(made({ kind: 'beforeRestore' }))).toBeNull();
    expect(checkpointBy(made({ kind: 'runStart', by: undefined, about: actorView('agent:x') }))).toBeNull();
    expect(checkpointBy(made({ by: undefined }))).toBeNull();
  });
  it('words what a refused checkpoint request means, in the context of a checkpoint', () => {
    const refused = (code: string): Event => ({ type: 'refused', code });
    const say = (code: string) => messageFor([refused(code)], 'checkpoint')?.text;
    expect(say('forbidden')).toBe('You may not make or restore checkpoints in this file.');
    expect(say('invalid')).toBe('That label cannot be used: a label is one line of plain text, at most 128 characters.');
    expect(say('checkpoint_full')).toMatch(/as many checkpoints/);
    expect(say('key_reused')).toMatch(/Try again/);
    expect(say('not_found')).toBe('That checkpoint is no longer on the server.');
    for (const code of ['forbidden', 'invalid', 'checkpoint_full', 'key_reused', 'not_found', 'too_far', 'checkpoint_failed', 'checkpoint_pending']) {
      const text = say(code);
      expect(text, code).toBeTruthy();
      expect(text, code).not.toMatch(/_/);
    }
  });
  it('keeps the words of the review for the same codes outside a checkpoint request', () => {
    expect(messageFor([{ type: 'refused', code: 'forbidden' }])?.text).toBe('You may not accept or flag batches in this file.');
    expect(messageFor([{ type: 'refused', code: 'checkpoint_full' }])?.text).toMatch(/as many checkpoints/);
  });
});

describe('Create checkpoint in words', () => {
  it('says nothing while the caller may make one, and says why not when it cannot, case by case', () => {
    expect(createWhyNot(true, 'loaded')).toBeNull();
    expect(createWhyNot(false, 'unread')).toBe('The checkpoints have not loaded yet.');
    expect(createWhyNot(false, 'absent')).toBe('This server does not keep checkpoints.');
    expect(createWhyNot(false, 'loaded')).toBe('This account may not make checkpoints in this file.');
  });
  it('does not claim a reason it does not know: no case blames the person for a list that is not there', () => {
    for (const status of ['unread', 'absent'] as const) expect(createWhyNot(false, status)).not.toMatch(/may not|person/);
  });
  it('tells assistive technology when the checkpoint is made', () => {
    expect(checkpointMade).toEqual({ tone: 'info', text: 'Checkpoint made.' });
  });
});
