// A place in a React tree where a view that is not React is drawn: the host gives a `Foreign` (a function that mounts the view on an element
// and can update and end it), and the slot mounts it when the slot appears, tells it the new props on every draw and ends it when the slot goes.
// It exists for as long as a panel is React and a part of it is not ported yet: the image picker of the Fill section is Svelte until S7, and
// it sits in the middle of the section, so it cannot be drawn beside the panel as the three dialogs of the Variables panel are. The slot is
// `display: contents`, so the foreign view's own element is the flex child of the section, as in the Svelte panel. It goes with S7.
import { useLayoutEffect, useRef } from 'react';

export interface Foreign<P> {
  /** Draws the view on `target` with `props`; the handle updates it with the next props and ends it. */
  mount(target: HTMLElement, props: P): { update(props: P): void; destroy(): void };
}

export function ForeignSlot<P>({ view, props }: { view: Foreign<P>; props: P }) {
  const host = useRef<HTMLDivElement>(null);
  const handle = useRef<ReturnType<Foreign<P>['mount']> | null>(null);
  const latest = useRef(props);
  useLayoutEffect(() => {
    latest.current = props;
    handle.current?.update(props);
  });
  useLayoutEffect(() => {
    const target = host.current;
    if (!target) return;
    const mounted = view.mount(target, latest.current);
    handle.current = mounted;
    return () => {
      handle.current = null;
      mounted.destroy();
    };
  }, [view]);
  return <div ref={host} className="contents" />;
}
