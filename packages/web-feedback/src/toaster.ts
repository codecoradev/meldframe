// The clock of the toast: one message at a time, shown for a while, replaced by the next. It holds no document state;
// the timer functions come in so a test can drive them by hand.

export type ToastTone = 'warning' | 'danger';

/** What the toast draws. `id` changes with every `show`, so the same text said twice is a new message. */
export interface ToastView {
  id: number;
  text: string;
  tone: ToastTone;
}

export interface Timers {
  set(fn: () => void, ms: number): unknown;
  clear(timer: unknown): void;
}

const browser: Timers = { set: (fn, ms) => setTimeout(fn, ms), clear: (t) => clearTimeout(t as ReturnType<typeof setTimeout>) };

export class Toaster {
  private timer: unknown;
  private count = 0;

  /** `ms` is how long a message stays (the `notice` duration token). */
  constructor(
    private onChange: (toast: ToastView | null) => void,
    private ms: number,
    private timers: Timers = browser,
  ) {}

  show(text: string, tone: ToastTone = 'warning'): void {
    this.stop();
    this.onChange({ id: ++this.count, text, tone });
    this.timer = this.timers.set(() => {
      this.timer = undefined;
      this.onChange(null);
    }, this.ms);
  }

  dismiss(): void {
    this.stop();
    this.onChange(null);
  }

  /** The page is going away: stop the timer and say nothing. */
  destroy(): void {
    this.stop();
  }

  private stop(): void {
    if (this.timer !== undefined) this.timers.clear(this.timer);
    this.timer = undefined;
  }
}
