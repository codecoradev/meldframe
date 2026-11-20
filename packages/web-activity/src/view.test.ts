import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { Checkpoint, Event } from '@meldframe/protocol';
import { activity, lane, row } from './fixtures';
import { batchHeading, checkpointWords, clockText, laneSummary, laneViews, messageFor, statusWords, summaryWords } from './view';

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
    const a = activity({ lanes: [lane({ actor: 'agent:claude' }), lane({ actor: 'person:ana', kind: 'person', batches: [] })] });
    const v = laneViews(a, 'person:ana');
    expect(v.map((l) => l.name)).toEqual(['claude', 'You']);
    expect(v.map((l) => l.self)).toEqual([false, true]);
  });

  it('gives an agent one of the three agent tones, always the same one, and a person none', () => {
    const a = activity({ lanes: [lane({ actor: 'agent:claude' }), lane({ actor: 'person:ana', kind: 'person', batches: [] })] });
    const [agent, person] = laneViews(a, 'x');
    expect([1, 2, 3]).toContain(agent!.tone);
    expect(laneViews(a, 'x')[0]!.tone).toBe(agent!.tone);
    expect(person!.tone).toBeNull();
  });

  it('counts a lane\'s batches as the app layer lists them, not by the rows that could be joined', () => {
    const a = activity({ lanes: [lane({ batches: ['k3', 'gone', 'k1'] })], rows: [row({ keyPrefix: 'k1' })] });
    expect(laneViews(a, 'x')[0]!.batches).toBe(3);
    expect(laneViews(a, 'x')[0]!.rows).toHaveLength(1);
  });

  it('draws the kind beside the name, so an agent called You is not the person and person:claude is not agent:claude', () => {
    const a = activity({
      lanes: [lane({ actor: 'agent:You', batches: [] }), lane({ actor: 'person:ana', kind: 'person', batches: [] }), lane({ actor: 'person:claude', kind: 'person', batches: [] }), lane({ actor: 'agent:claude', batches: [] })],
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
  const cp = (over: Partial<Checkpoint>): Checkpoint => ({ id: 'c1', seq: 42, label: '', kind: 'manual', ...over });
  it('uses the label the person typed, and words the checkpoints the app took by themselves', () => {
    expect(checkpointWords(cp({ label: 'Before the pitch' }))).toBe('Before the pitch');
    expect(checkpointWords(cp({}))).toBe('Checkpoint');
    expect(checkpointWords(cp({ kind: 'otherActorRun', actor: 'agent:claude' }))).toBe("Before claude's run");
    expect(checkpointWords(cp({ kind: 'beforeRevert', actor: 'agent:claude' }))).toBe("Before reverting claude's batch");
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
    for (const src of [rust('app/meldframe-app/src/review.rs'), rust('app/meldframe-app/src/activity.rs')]) for (const m of src.matchAll(/refuse\(\s*&mut self\.events,\s*"([a-z_]+)"\s*\)/g)) found.add(m[1]!);
    for (const m of rust('core/meldframe-activity/src/feed.rs').matchAll(/ok_or\("([a-z_]+)"\)/g)) found.add(m[1]!);
    return [...found].sort();
  };
  it('reads the codes from the sources, and finds the ones the review flows refuse with', () => {
    expect(appCodes()).toEqual(expect.arrayContaining(['batch_incomplete', 'limit', 'log_mismatch', 'no_batch_number', 'not_in_log', 'nothing_to_restore', 'nothing_to_revert', 'unknown_batch', 'unknown_checkpoint']));
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
