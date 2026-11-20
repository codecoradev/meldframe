// The Activity panel as words and rows: what to call things and which rows go under which lane. Pure. The app layer
// has already decided the lanes, their order, the counts and which row may be reverted; nothing here decides, it joins
// and words what `state.activity` says (ADR-0021).
import type { ActivityLane, ActivityState, BatchRow, BatchSummary, Checkpoint, Event, LaneKind, LaneState, ReviewState } from '@meldframe/protocol';
import { actorLabel, actorTone } from '@meldframe/web-document/layers';

export interface LaneView {
  actor: string;
  /** What the lane is called: the actor's name, or You for the actor of this page. */
  name: string;
  kind: LaneKind;
  /** Always drawn beside the name: a bare name can look like another lane's (an agent called You, or `person:claude` and `agent:claude`). */
  kindWords: 'Agent' | 'Person';
  self: boolean;
  /** One of the three agent colours, from the actor; null for a person (the person has its own colour). */
  tone: 1 | 2 | 3 | null;
  stateWords: string;
  ops: number;
  /** How many batches the lane has, as the app layer counts them (not the rows that could be joined). */
  batches: number;
  runs: ActivityLane['runs'];
  /** The lane's batches, newest first, in the order the app layer gave. */
  rows: BatchRow[];
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const STATE: Record<LaneState, string> = { working: 'Working', quiet: 'Quiet', idle: 'Idle' };
// A batch is its actor and its key prefix: two actors may use the same prefix.
const id = (actor: string, keyPrefix: string) => `${actor}\u0000${keyPrefix}`;

/** The lanes with their rows. A prefix the lane lists but the rows do not hold (cut at the row limit) is left out. */
export function laneViews(a: ActivityState, self: string): LaneView[] {
  const byId = new Map(a.rows.map((r) => [id(r.actor, r.keyPrefix), r]));
  return a.lanes.map((l) => ({
    actor: l.actor,
    name: l.actor === self ? 'You' : actorLabel(l.actor).name,
    kind: l.kind,
    kindWords: l.kind === 'agent' ? 'Agent' : 'Person',
    self: l.actor === self,
    tone: l.kind === 'agent' ? actorTone(l.actor) : null,
    stateWords: STATE[l.state],
    ops: l.ops,
    batches: l.batches.length,
    runs: l.runs,
    rows: l.batches.flatMap((k) => byId.get(id(l.actor, k)) ?? []),
  }));
}

/** The line under a lane's name: its kind, its state and its counts, as the app layer gives them. */
export function laneSummary(l: Pick<LaneView, 'kindWords' | 'stateWords' | 'ops' | 'batches'>): string {
  const parts = [l.kindWords, l.stateWords.toLowerCase(), plural(l.ops, 'change')];
  if (l.batches > 0) parts.push(plural(l.batches, 'batch', 'batches'));
  return parts.join(', ');
}

export const batchHeading = (r: BatchRow): string => (r.n === null ? 'Batch without a number' : `Batch ${r.n}`);

export function statusWords(r: BatchRow): string {
  switch (r.status) {
    case 'applied': return 'Applied';
    case 'partial': return `${r.written} of ${r.commands} applied`;
    case 'building': return 'Arriving';
    case 'headerless': return 'No header: no number, note or review';
  }
}


/** "2 added, 1 changed: Hero, Title"; null for a row made from a header alone (its ops are not held). */
export function summaryWords(s: BatchSummary | null): string | null {
  if (!s) return null;
  const counts = [[s.added, 'added'], [s.changed, 'changed'], [s.removed, 'removed']].filter(([n]) => n !== 0).map(([n, w]) => `${n} ${w}`);
  if (counts.length === 0) return 'No node changed';
  return s.names.length ? `${counts.join(', ')}: ${s.names.join(', ')}` : counts.join(', ');
}

/** The time of day in the zone of the page, as the board writes it: 10:42:31. */
export function clockText(ms: number): string {
  const d = new Date(ms);
  const two = (n: number) => String(n).padStart(2, '0');
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}

/** What a checkpoint is called: the person's label, or words for the ones the app took by itself. */
export function checkpointWords(c: Checkpoint): string {
  const who = c.actor ? actorLabel(c.actor).name : 'an actor';
  switch (c.kind) {
    case 'manual': return c.label.trim() || 'Checkpoint';
    case 'otherActorRun': return `Before ${who}'s run`;
    case 'beforeRevert': return `Before reverting ${who}'s batch`;
    case 'beforeRestore': return 'Before a restore';
  }
}

export interface Message {
  tone: 'info' | 'warning';
  text: string;
}

// The codes the app layer refuses a revert, a restore, Show or a review with (review.rs, activity.rs, feed.rs), and the
// codes the batch routes answer with (ADR-0021: forbidden, not_found, invalid, rate_limited; `unavailable` when the
// request did not get through). A test reads the Rust sources, so a code added there without words fails here.
const REFUSALS: Record<string, string> = {
  forbidden: 'You may not accept or flag batches in this file.',
  unknown_batch: 'That batch is older than this page: it cannot be shown or reverted here.',
  no_batch_number: 'A batch with no number cannot be reviewed.',
  nothing_to_revert: 'Nothing to revert: every field that batch wrote was written again since, or it was reverted already.',
  batch_incomplete: 'This page does not hold every change of that batch, so it cannot be reverted here.',
  log_mismatch: "This page's record of the file no longer fits the server's. Reload the page, then try again.",
  nothing_to_restore: 'The file is already as that checkpoint has it.',
  not_in_log: "That checkpoint is older than this page's record, so it cannot be restored here.",
  unknown_checkpoint: 'That checkpoint no longer exists.',
  limit: 'That is more than there can be.',
  not_found: 'The server has no such batch.',
  invalid: 'The server did not take that review state.',
  rate_limited: 'Too fast: wait a moment and try again.',
  unavailable: 'The server could not be reached. Try again in a moment.',
};

/**
 * The last thing the panel should say about the events of one dispatch (a refusal, a revert, a restore), or null. A
 * refusal it has no words for is left to the page's toast, which words any code; the panel never prints a raw code.
 */
export function messageFor(events: readonly Event[]): Message | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i]!;
    if (e.type === 'refused') {
      const text = Object.hasOwn(REFUSALS, e.code) ? REFUSALS[e.code] : undefined;
      if (text) return { tone: 'warning', text };
      continue;
    }
    if (e.type === 'reverted') {
      const left = e.skipped.length ? `, ${plural(e.skipped.length, 'part')} left as they are` : '';
      return { tone: 'info', text: `Reverted: ${plural(e.requested, 'change')} sent${left}. One undo takes them back.` };
    }
    if (e.type === 'restored') return { tone: 'info', text: `Restored: ${plural(e.requested, 'change')} sent. One undo takes them back.` };
  }
  return null;
}

const REVIEW: Record<ReviewState, string> = { unreviewed: 'Not reviewed', accepted: 'Accepted', needs_review: 'Needs review' };

/** The review state of a batch in words: everyone sees it, and it is never only a colour. */
export const reviewWords = (s: ReviewState): string => REVIEW[s];

/**
 * Who last changed the state and when: the full actor, because a bare name can be another actor's (`person:claude`
 * and `agent:claude`), and the page draws it in an isolate. After a clear to unreviewed the server still names the
 * person who cleared it, so the page says "last changed by", and never "unreviewed by" as if that person were a reviewer.
 */
export function reviewedBy(r: BatchRow): { by: string; at: string | null } | null {
  return r.reviewedBy === null ? null : { by: r.reviewedBy, at: r.reviewedAtMs === null ? null : clockText(r.reviewedAtMs) };
}

export interface ReviewChoice {
  state: ReviewState;
  label: string;
}

/**
 * The controls of one row: the states other than its own. The web hides a control the app layer would refuse
 * (`canReview` from the server; no number, `no_batch_number`) and decides nothing else about who may review.
 */
export function reviewChoices(r: BatchRow, canReview: boolean): ReviewChoice[] {
  if (!canReview || r.n === null) return [];
  const all: ReviewChoice[] = [
    { state: 'accepted', label: 'Accept' },
    { state: 'needs_review', label: 'Needs review' },
    { state: 'unreviewed', label: 'Clear' },
  ];
  return all.filter((c) => c.state !== r.review);
}
