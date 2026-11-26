// The React side of the focus handoff (`../handoff.ts`): one `Handoff` for one overlay, told when the overlay opens. It is
// the only place that reads `document.activeElement`. A layout effect on `open` runs before Base UI moves the focus (its
// popup mounts in a later render), once per open; `open` is the overlay's own state, so a menu that Base UI would
// open by itself is given one (`Menu.tsx`).
//
//   const hand = useFocusHandoff(open);
//   <Popup finalFocus={hand.finalFocus} />                     the return, decided and not spent here
//   <Root onOpenChangeComplete={(o) => !o && hand.closed()} /> only for an overlay whose row or entry calls `hand.pass`
import { useLayoutEffect, useState } from 'react';
import { type Handoff, createHandoff } from '../handoff';

export function useFocusHandoff(open: boolean): Handoff<HTMLElement> {
  const [handoff] = useState(() => createHandoff<HTMLElement>());
  useLayoutEffect(() => {
    if (open) handoff.opened(document.activeElement as HTMLElement | null);
  }, [open, handoff]);
  return handoff;
}
