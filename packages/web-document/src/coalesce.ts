// At most one piece of work per animation frame, on the latest value: a pointer reports faster than the screen draws, and
// what a drag shows (the slot the app layer names, which costs a query) only has to be right when a frame is drawn.
// Pure: the clock of frames is injected (`requestAnimationFrame` in the page, a hand-turned one in a test).

/** The two calls of an animation frame clock. */
export interface Frames {
  request(cb: () => void): number;
  cancel(id: number): void;
}

export interface PerFrame<T> {
  /** Remember `value` as the latest and have it run at the next frame (the one already waiting is reused). */
  push(value: T): void;
  /** Run the waiting value now, if there is one, and drop its frame (the gesture ended: nothing may lag behind it). */
  flush(): void;
  /** Forget the waiting value and its frame. */
  cancel(): void;
}

export function perFrame<T>(frames: Frames, run: (value: T) => void): PerFrame<T> {
  let waiting: { value: T } | null = null;
  let handle: number | null = null;
  const drop = () => {
    if (handle !== null) frames.cancel(handle);
    handle = null;
  };
  const take = () => {
    const w = waiting;
    waiting = null;
    return w;
  };
  return {
    push(value) {
      waiting = { value };
      handle ??= frames.request(() => {
        handle = null;
        const w = take();
        if (w) run(w.value);
      });
    },
    flush() {
      drop();
      const w = take();
      if (w) run(w.value);
    },
    cancel() {
      drop();
      waiting = null;
    },
  };
}
