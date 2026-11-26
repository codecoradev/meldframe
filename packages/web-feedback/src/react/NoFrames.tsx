// React copy of NoFrames.svelte. The empty state of a file with no frame: what to do about it, and a button that adds
// one. Shown once the canvas is ready (a canvas that has not started has nothing to say yet). The page decides what
// "add" does.
import { Button } from '@meldframe/web-chrome/react/Button';
import { hasFrames } from '../empty';
import { EmptyState } from './EmptyState';

export interface NoFramesProps {
  nodes: readonly { type: string }[];
  ready: boolean;
  /** False while the page is out of step with the server: the app ignores edits then. */
  canAdd: boolean;
  onAdd: () => void;
}

export function NoFrames({ nodes, ready, canAdd, onAdd }: NoFramesProps) {
  if (!ready || hasFrames(nodes)) return null;
  return (
    <EmptyState title="No frames yet" text="Press F and drag to draw a frame, or ask an agent connected over MCP to build a screen.">
      <Button variant="primary" disabled={!canAdd} onClick={onAdd}>
        Add a frame
      </Button>
    </EmptyState>
  );
}
