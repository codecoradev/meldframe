// React copy of RemoveConfirm.svelte. The question before a removal: what will happen, in numbers, and two answers. The safe one
// (Cancel) takes the focus when this opens; Escape cancels from anywhere inside it and does not close the dialog around it. The
// words are the removal words (`removalWords`); what a click does is the parent's. The parent gives it a `key` for what is asked,
// so a question about another thing starts again and takes the focus on Cancel as the first one did.
import { Button } from '@meldframe/web-chrome/react/Button';
import { useEffect, useRef } from 'react';
import type { RemovalWords } from '../manage';

export interface RemoveConfirmProps {
  plan: RemovalWords;
  onConfirm: () => void;
  onCancel: () => void;
}

export function RemoveConfirm({ plan, onConfirm, onCancel }: RemoveConfirmProps) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => root.current?.querySelector<HTMLElement>('[data-testid=remove-cancel]')?.focus(), []);
  return (
    // Escape closes the question from any control inside it; the group is not a control itself.
    <div
      ref={root}
      role="alertdialog"
      tabIndex={-1}
      aria-label="Confirm the removal"
      aria-describedby="remove-confirm-text"
      className="flex flex-col gap-2 rounded-md border border-border bg-surface px-3 py-2"
      data-testid="remove-confirm"
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onCancel();
        }
      }}
    >
      <p id="remove-confirm-text" className="m-0 text-small text-text break-words">
        {plan.text}
      </p>
      <div className="flex gap-2">
        <Button size="sm" onClick={onCancel} data-testid="remove-cancel">
          Cancel
        </Button>
        <Button size="sm" variant="destructive" onClick={onConfirm} data-testid="remove-confirm-yes">
          {plan.confirm}
        </Button>
      </div>
    </div>
  );
}
