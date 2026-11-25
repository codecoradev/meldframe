// React copy of Field.svelte. A field: a labelled text input with an optional prefix and suffix. `label` is the
// accessible name (the prefix is a shorthand such as X or W). Hover, focus, invalid and variable-bound outlines follow
// the state matrix; the ring shows when the input has focus. The value is the caller's: it reports input and commits
// on Enter or blur, Escape puts the value back, and after a commit the input shows the caller's value again (a value
// the caller did not take is undone). The keys typed here are text, not commands of the page.
import { type ReactNode, useEffect, useLayoutEffect, useRef } from 'react';
import { dim, type FieldSize, fieldHeight, requireLabel, ringWithin } from '../focus';

export interface FieldProps {
  label: string;
  value: string;
  onInput?: (value: string) => void;
  onCommit?: (value: string) => void;
  prefix?: string;
  suffix?: string;
  size?: FieldSize;
  disabled?: boolean;
  invalid?: boolean;
  /** Bound to a Variable: chip colours, the name is the value. */
  bound?: boolean;
  /** Something drawn before the input, such as a colour swatch. */
  lead?: ReactNode;
  /** The value in the mono face (numbers, ids, variable values). */
  mono?: boolean;
  /** Shown while the field is empty, such as `Mixed` for a value the parts of a whole do not share. */
  placeholder?: string;
  /** `data-testid` of the input. */
  testId?: string;
  /** The id of the element that explains the field (why it is disabled, for one): its description. */
  describedBy?: string;
  [data: `data-${string}`]: string | undefined;
}

const looks = {
  invalid: 'border-danger bg-danger-bg text-danger-text',
  bound: 'border-transparent bg-chip-bg text-chip-text hov:border-chip-border',
  plain: 'border-divider bg-field text-text hov:border-border-strong foc-in:border-selection',
};

export function Field({ label, value, onInput, onCommit, prefix, suffix, size = 'default', disabled = false, invalid = false, bound = false, lead, mono = false, placeholder, testId, describedBy, ...rest }: FieldProps) {
  const name = requireLabel(label, 'Field');
  const look = invalid ? looks.invalid : bound ? looks.bound : looks.plain;
  const input = useRef<HTMLInputElement>(null);
  // What the native `change` listener below reads: React draws the input as the caller's value, a commit reports the typed one.
  const latest = useRef({ value, onCommit });
  useLayoutEffect(() => {
    latest.current = { value, onCommit };
  });
  useLayoutEffect(() => {
    // The caller's value wins when it changes, and only then (Svelte sets the property the same way): a parent that
    // draws again while the person types, because `onInput` set a flag, must not put the old value back mid-keystroke.
    if (input.current && input.current.value !== value) input.current.value = value;
  }, [value]);
  useEffect(() => {
    const el = input.current;
    if (!el) return;
    // The native `change`: it fires on Enter and on blur, and only when the person typed something (React's onChange is every key).
    const commit = () => {
      latest.current.onCommit?.(el.value);
      // After the commit the input shows the caller's value again; a value the caller did not take is undone.
      queueMicrotask(() => {
        el.value = latest.current.value;
      });
    };
    el.addEventListener('change', commit);
    return () => el.removeEventListener('change', commit);
  }, []);

  return (
    <label {...rest} className={`${fieldHeight[size]} ${ringWithin} ${disabled ? dim : ''} flex min-w-0 items-center gap-2 rounded-md border px-3 text-small ${look}`}>
      {prefix ? (
        <span className="shrink-0 text-text-muted" aria-hidden="true">
          {prefix}
        </span>
      ) : null}
      {lead}
      <input
        ref={input}
        data-testid={testId}
        className={`${mono ? 'font-mono' : 'font-[inherit]'} min-w-0 flex-1 border-0 bg-transparent p-0 text-[inherit] outline-none`}
        aria-label={name}
        aria-invalid={invalid || undefined}
        aria-describedby={describedBy}
        defaultValue={value}
        placeholder={placeholder}
        disabled={disabled}
        onInput={(e) => onInput?.(e.currentTarget.value)}
        onKeyDown={(e) => {
          const el = e.currentTarget;
          if (e.key === 'Enter') el.blur();
          else if (e.key === 'Escape') {
            el.value = value;
            el.blur();
          }
          e.stopPropagation();
        }}
      />
      {suffix ? (
        <span className="shrink-0 text-text-muted" aria-hidden="true">
          {suffix}
        </span>
      ) : null}
    </label>
  );
}
