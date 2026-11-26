// React copy of ActivityPanel.svelte (first part: the panel, its lanes and the status line; the rows of a batch and the Checkpoints
// section follow in the second part, and until then this copy is not reachable from the editor). The Activity panel (E5.1,
// ADR-0021): one lane per actor, then the checkpoints. It draws `state.activity` of the app layer and sends actions: the lanes,
// their order, the counts and which row may be reverted are the app layer's, and so is the question being asked
// (`AppState.confirm`). What it keeps itself is view state only (which lanes the person folded or opened, the line it says and
// when it goes), and what a control sends is `actions.ts`, shared with the Svelte copy, as are the two rules in `controller.ts`.
// A text with a value in it is one string (a template), not two text nodes.
import type { Action, ActivityState, ActorTone, Checkpoint, CheckpointsStatus, Confirm, Event } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Icon } from '@meldframe/web-chrome/react/Icon';
import { Section } from '@meldframe/web-chrome/react/Section';
import { ring } from '@meldframe/web-chrome/focus';
import { duration } from '@meldframe/web-tokens/tokens';
import { useCallback, useEffect, useRef, useState } from 'react';
import { older } from '../actions';
import { type About, aboutAfter, eventsSay, laneOpen, start, toggleLane, type Ui } from '../controller';
import { clockText, laneSummary, laneViews, type Message } from '../view';
import { MessageLine } from './MessageLine';

export interface ActivityPanelProps {
  activity: ActivityState;
  checkpoints: readonly Checkpoint[];
  /** Whether this caller may make a checkpoint (`AppState.canCreateCheckpoint`). */
  canCreate: boolean;
  /** Whether the list was read and whether the server keeps checkpoints (`AppState.checkpointsStatus`). */
  status: CheckpointsStatus;
  /** The actor of this page: its lane is called You. */
  self: string;
  /** The question being asked (`AppState.confirm`), or none. */
  confirm?: Confirm | null;
  dispatch: (a: Action) => void;
  /**
   * The events of the last dispatch. A refusal, a revert or a restore that comes while the panel is open is worded
   * (`messageFor`) and shown for a few seconds; what was already there when the panel opened is not.
   */
  events?: readonly Event[];
}

// Presentation only: the tone is the app layer's (`ActorView.tone`), this is the class that draws it.
const TONE: Record<ActorTone, string> = { person: 'bg-person', agent1: 'bg-agent-1', agent2: 'bg-agent-2', agent3: 'bg-agent-3' };
const changes = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;
const none: readonly Event[] = [];

export function ActivityPanel({ activity, self, dispatch, events = none }: ActivityPanelProps) {
  const lanes = laneViews(activity, self);
  const [ui, setUi] = useState<Ui>(start);
  // What the person asked for last decides the words of a refusal: `forbidden` means one thing for a review and another for a checkpoint.
  const about = useRef<About>(undefined);
  const send = (a: Action) => {
    about.current = aboutAfter(about.current, a);
    dispatch(a);
  };

  const [message, setMessage] = useState<Message | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const say = useCallback((m: Message) => {
    setMessage(m);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setMessage(null), duration.notice);
  }, []);
  // The events that were there when the panel opened are not said again: only a later dispatch (another array) is.
  const seen = useRef(events);
  useEffect(() => {
    if (events === seen.current) return;
    seen.current = events;
    const says = eventsSay(events, about.current);
    if (says.message) say(says.message);
  }, [events, say]);
  useEffect(() => () => clearTimeout(timer.current), []);

  const count = activity.unreviewedCount;
  return (
    <div className="flex flex-col" data-testid="activity">
      <MessageLine message={message} />
      <Section title="Recent work">
        {count === null ? (
          <p className="m-0 text-small text-text-muted" data-testid="review-count" data-unknown="true">
            Batches to review: Unknown. The list of batches has not loaded.
          </p>
        ) : count > 0 ? (
          <p className="m-0 text-small text-text-secondary" data-testid="review-count">{`${count} ${count === 1 ? 'batch' : 'batches'} to review`}</p>
        ) : null}
        {lanes.length === 0 ? (
          <p className="m-0 text-small text-text-muted" data-testid="activity-empty">
            Nothing yet. When an agent or a person changes the file, the work shows here, one lane for each.
          </p>
        ) : null}
        {lanes.map((l, i) => {
          const open = laneOpen(ui, l.actor, i);
          return (
            <div key={l.actor} className="flex flex-col gap-2" data-testid="lane" data-actor={l.actor} data-state={l.stateWords.toLowerCase()}>
              <button
                type="button"
                aria-expanded={open}
                aria-controls={`lane-body-${i}`}
                onClick={() => setUi(toggleLane(ui, l.actor, open))}
                className={`${ring} flex min-h-(--mf-size-control-lg) w-full flex-col justify-center rounded-md py-1 text-left hov:bg-control`}
              >
                <span className="flex w-full items-center gap-2">
                  <span className="text-icon-muted">
                    <Icon name={open ? 'chevron-down' : 'chevron-right'} />
                  </span>
                  <span className={`size-2 shrink-0 rounded-pill ${TONE[l.tone]}`} data-tone={l.tone} aria-hidden="true"></span>
                  <span className="min-w-0 flex-1 truncate text-body font-semibold text-text-strong" title={l.actor}>
                    <bdi>{l.name}</bdi>
                  </span>
                </span>
                <span className="pl-12 text-caption text-text-muted" data-testid="lane-summary">
                  {laneSummary(l)}
                </span>
              </button>
              {open ? (
                <div id={`lane-body-${i}`} className="flex flex-col gap-2">
                  {l.rows.length === 0 ? (
                    <ul className="m-0 flex list-none flex-col gap-1 p-0 pl-6 text-small text-text-secondary" data-testid="lane-runs">
                      {l.runs.map((stretch) => (
                        <li key={stretch.startMs}>{`${clockText(stretch.startMs)} to ${clockText(stretch.endMs)}: ${changes(stretch.ops)}`}</li>
                      ))}
                    </ul>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
        {activity.hasOlder ? (
          <Button size="sm" onClick={() => send(older())} data-testid="older">
            Show older batches
          </Button>
        ) : null}
      </Section>
    </div>
  );
}
