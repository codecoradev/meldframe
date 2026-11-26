// React copy of CheckpointList.svelte. The checkpoints of this page, newest first, each with Restore behind a confirmation. A
// checkpoint is the file as the server confirmed it after change `seq`; a restore adds new changes at the end of the log and never
// rewrites it. Which checkpoint is being asked about is the app layer's (`AppState.confirm`): this draws it and moves the keyboard
// focus.
import type { Checkpoint } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { useEffect, useRef } from 'react';
import { checkpointBy, checkpointWords } from '../view';

export interface CheckpointListProps {
  /** Newest first, as the app keeps them. */
  checkpoints: readonly Checkpoint[];
  /** The id of the checkpoint a Restore is being confirmed for. */
  asking: string | null;
  onAsk: (id: string) => void;
  onRestore: (id: string) => void;
  onCancel: () => void;
}

export function CheckpointList({ checkpoints, asking, onAsk, onRestore, onCancel }: CheckpointListProps) {
  const list = useRef<HTMLDivElement>(null);
  const button = (id: string, testid: string) => list.current?.querySelector<HTMLElement>(`[data-checkpoint="${CSS.escape(id)}"] [data-testid="${testid}"] button`);
  // The question takes the focus (on its safe button), and Restore gets it back when the question closes.
  const before = useRef<string | null>(null);
  useEffect(() => {
    const was = before.current;
    before.current = asking;
    if (asking !== null) button(asking, 'restore-cancel')?.focus();
    else if (was !== null) button(was, 'checkpoint-restore')?.focus();
  }, [asking]);

  return (
    <div ref={list} data-testid="checkpoints">
      {checkpoints.length === 0 ? (
        <p className="m-0 text-small text-text-muted">No checkpoint yet. The server makes one when an agent&apos;s run starts, and one goes first before a revert or a restore.</p>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {checkpoints.map((c) => {
            const by = checkpointBy(c);
            return (
              <li key={c.id} className="flex flex-col gap-2 rounded-md border border-hairline bg-field px-3 py-2" data-testid="checkpoint" data-checkpoint={c.id}>
                <div className="flex items-baseline justify-between gap-2">
                  <span className="line-clamp-2 min-w-0 break-words text-small font-medium text-text-strong" title={checkpointWords(c)}>
                    <bdi>{checkpointWords(c)}</bdi>
                  </span>
                  <span className="shrink-0 text-caption text-text-muted">{`After change ${c.seq}`}</span>
                </div>
                {by ? (
                  <span className="text-caption text-text-muted" data-testid="checkpoint-by">
                    <bdi>{by}</bdi>
                  </span>
                ) : null}
                {asking === c.id ? (
                  // Escape closes the question from either button; the group is not a control itself.
                  <div
                    role="group"
                    aria-label="Confirm the restore of this checkpoint"
                    className="flex flex-col gap-2"
                    data-testid="restore-confirm"
                    onKeyDown={(e) => {
                      if (e.key === 'Escape') {
                        e.stopPropagation();
                        onCancel();
                      }
                    }}
                  >
                    <p className="m-0 text-small text-text">Restore the file to this point? New changes at the end of the log put it back; nothing is rewritten.</p>
                    <div className="flex gap-2">
                      <Button size="sm" variant="destructive" onClick={() => onRestore(c.id)} data-testid="restore-yes">
                        Restore
                      </Button>
                      <span data-testid="restore-cancel" className="contents">
                        <Button size="sm" onClick={onCancel}>
                          Cancel
                        </Button>
                      </span>
                    </div>
                  </div>
                ) : (
                  <span className="self-start" data-testid="checkpoint-restore">
                    <Button size="sm" onClick={() => onAsk(c.id)}>
                      Restore
                    </Button>
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
