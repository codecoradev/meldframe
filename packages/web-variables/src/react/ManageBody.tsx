// React copy of ManageBody.svelte. The manage view of the Variables (D8, the VariableManagement board): the collections on the left,
// the chosen one on the right with its modes and variables, and the question before a removal. It reads the panel and the manage state
// the app layer gives it (`AppState.variables`, `AppState.manage`) and sends actions: the collection shown, the question before a
// removal, the rename field and the text of the three forms are the app layer's (finding T5), and so are the checks of a name and what
// a removal detaches. The ids are the app layer's too, so the thing that was added is found by `Event::EntityAdded` and given the
// focus. What this holds is drawing state only: the text of the last refusal or change (the live regions), the words of the change
// just sent, and where the focus goes back to. What to do with the events of a dispatch is `reactTo` and `settle` (`../manage.ts`) and
// where the focus goes is `../askers.ts`, which the Svelte copy calls too.
//
// The Svelte copy waits for its own next flush (`tick`) before it moves the focus after a dispatch. Here the answer reaches the
// component as new props, in a render of its own that React schedules later than the click, so a focus that must follow a dispatch is
// kept in `after` and runs in the effect that follows the props changing; the focus that follows the events of a dispatch runs in the
// effect that reads them, which already has the page the events belong to. These are layout effects: they run after the page is
// updated and before it is painted, and a state they set (the words of a refusal) is drawn before the paint too, so the field that
// closed and the words that say why appear together, as they do in the Svelte copy (with passive effects, one run of manage-e2e.mjs read
// the error line after the field had closed and before the words were in it).
import type { Action, Event, ManageOp, ManageState, ManageTarget, VarType, VariableEntity, VariablesPanel } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { useLayoutEffect, useRef, useState } from 'react';
import { focusAsker, focusEntity } from '../askers';
import { collectionSummary, partial, reactTo, removalKey, removalWords, settle } from '../manage';
import { CollectionDetail } from './CollectionDetail';
import { CollectionList } from './CollectionList';
import { RemoveConfirm } from './RemoveConfirm';

export interface ManageBodyProps {
  panel: VariablesPanel;
  /** `AppState.manage`. */
  manage: ManageState;
  /** The events of the last dispatch (`snapshot.events`). */
  events: readonly Event[];
  dispatch: (a: Action) => void;
}

export function ManageBody({ panel, manage, events, dispatch }: ManageBodyProps) {
  const root = useRef<HTMLDivElement>(null);
  const current = panel.collections.find((c) => c.id === manage.shown) ?? null;
  const summary = current ? collectionSummary(current, panel) : null;
  const why = partial(panel);
  const removal = manage.removal ?? null;
  const words = removal ? removalWords(removal) : null;
  const rename = manage.rename ?? null;

  // The live regions are always on the page and the text is put into them (a region that appears with its text is not announced).
  const [error, setError] = useState('');
  const [status, setStatus] = useState('');
  // The words of the change just sent, said when the document answers; what was sent last (the words of a limit depend on it); the
  // thing a removal asked about, to give the focus back to after it; the last thing a rename field was open on, whose button gets the focus back when the field closes.
  const pending = useRef<string | null>(null);
  const sent = useRef<Action | null>(null);
  const refocus = useRef<ManageTarget | null>(null);
  const lastRename = useRef<ManageTarget | null>(null);
  // The focus to give once the page shows the answer of the dispatch just made.
  const after = useRef<(() => void) | null>(null);
  const seen = useRef(events);

  useLayoutEffect(() => {
    if (manage.rename) lastRename.current = manage.rename.target;
  });

  useLayoutEffect(() => {
    if (events === seen.current) return;
    seen.current = events;
    const s = settle(reactTo(events, pending.current, manage.limits, sent.current), lastRename.current, refocus.current);
    if (s.error !== null) setError(s.error);
    if (s.status !== null) setStatus(s.status);
    if (s.pending) pending.current = null;
    if (s.refocused) refocus.current = null;
    for (const f of s.focus) {
      if ('entity' in f) focusEntity(root.current, f.entity);
      else focusAsker(root.current, f.asker, f.attr);
    }
  }, [events, manage.limits]);

  // `panel` and `manage` are what a dispatch changes: when they arrive, the thing the focus was promised to is on the page. They are only
  // the trigger of this effect (it reads the ref), and no state of this component is: a re-render of its own does not run it.
  // It runs on the next change of either, whichever snapshot brings it: that is the answer of the dispatch just made, because the engine
  // answers a dispatch at once, so every snapshot after it already holds the answer, and `back` is only asked for when the thing to leave (an open question or field)
  // is there, so the dispatch does change `manage`. A dispatch that changed nothing would leave the focus waiting for the next change.
  useLayoutEffect(() => {
    const f = after.current;
    after.current = null;
    f?.();
  }, [panel, manage]);

  const op = (o: ManageOp) => dispatch({ type: 'manage', op: o });
  const send = (a: Action, done: string) => {
    pending.current = done;
    sent.current = a;
    dispatch(a);
  };
  const back = (a: ManageTarget | undefined, attr: 'remove' | 'rename') => {
    if (a) after.current = () => focusAsker(root.current, a, attr);
  };
  const close = () => {
    back(removal?.target, 'remove');
    op({ type: 'cancelRemove' });
  };
  // A cancel is quiet: the words of an earlier change do not stay in the status line as if this had done something.
  const cancelRename = () => {
    back(rename?.target, 'rename');
    op({ type: 'cancelRename' });
    setStatus('');
    setError('');
  };
  const submitRename = () => {
    const quiet = rename?.unchanged;
    pending.current = null;
    sent.current = null;
    // A name that is no change closes the field and says nothing: a quiet cancel.
    if (quiet) back(rename?.target, 'rename');
    op({ type: 'submitRename' });
    if (quiet) {
      setStatus('');
      setError('');
    }
  };
  const confirm = () => {
    refocus.current = removal?.target ?? null;
    pending.current = words?.done ?? null;
    sent.current = null;
    op({ type: 'confirmRemove' });
  };
  const draft = (form: VariableEntity, text: string) => op({ type: 'setDraft', form, text });

  return (
    <div ref={root} className="flex min-h-0 flex-1 flex-col" data-testid="manage">
      {why ? (
        <p className="m-0 flex items-center justify-between gap-3 border-b border-hairline bg-warning-bg px-4 py-2 text-small text-text-strong" data-testid="manage-partial">
          {why === 'search' ? (
            <>
              <span>The search of the Variables panel narrows this list, so the counts cover only what it matches.</span>
              <Button size="sm" onClick={() => dispatch({ type: 'setVariablesQuery', query: '' })} data-testid="manage-clear-search">
                Clear search
              </Button>
            </>
          ) : (
            <span>This file has more variables than the list holds, so some counts are partial.</span>
          )}
        </p>
      ) : null}
      <div className="flex min-h-0 flex-1">
        <div className="w-64 shrink-0 overflow-y-auto border-r border-hairline">
          <CollectionList
            collections={panel.collections}
            chosen={current?.id ?? null}
            onChoose={(id) => op({ type: 'choose', id })}
            draft={manage.drafts.collection}
            limits={manage.limits}
            onInput={(text) => draft('collection', text)}
            onAdd={(n) => send({ type: 'createCollection', name: n }, `Added the collection “${n}”.`)}
          />
        </div>
        <div className="min-w-0 flex-1 overflow-y-auto" data-testid="manage-detail">
          {removal && words ? (
            // A question for another thing starts again, so it takes the focus on Cancel as the first one did.
            <div key={removalKey(removal)} className="px-4 pt-4">
              <RemoveConfirm plan={words} onConfirm={confirm} onCancel={close} />
            </div>
          ) : null}
          {current && summary ? (
            <CollectionDetail
              col={current}
              summary={summary}
              onAsk={(t) => op({ type: 'askRemove', target: t })}
              rename={rename}
              drafts={manage.drafts}
              limits={manage.limits}
              onRename={(t) => op({ type: 'startRename', target: t })}
              onRenameInput={(text) => op({ type: 'setRenameText', text })}
              onRenameSubmit={submitRename}
              onRenameCancel={cancelRename}
              onDraft={draft}
              onAddMode={(n, from) => send({ type: 'addMode', collection: current.id, name: n, ...(from ? { copyFrom: from } : {}) }, `Added the mode “${n}”.`)}
              onAddVariable={(n, kind: VarType) => send({ type: 'addVariable', collection: current.id, name: n, kind }, `Added the variable “${n}”.`)}
            />
          ) : null}
        </div>
      </div>
      <div className="border-t border-hairline px-4 py-2">
        <div role="alert" data-testid="manage-error">
          {error ? <p className="m-0 rounded-md bg-danger-bg px-3 py-2 text-small text-danger-text">{error}</p> : null}
        </div>
        <div role="status" data-testid="manage-status">
          {status && !error ? <p className="m-0 px-1 text-small text-text-secondary">{status}</p> : null}
        </div>
      </div>
    </div>
  );
}
