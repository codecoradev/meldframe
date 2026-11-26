// React copy of Button.svelte. A text button: primary (accent-strong), secondary (control) or destructive
// (danger-strong). White text sits on the strong fills only (5.17:1); the brand blue carries no text. Class strings
// are whole, never built from a prop, so that Tailwind finds them.
import type { MouseEvent, ReactNode } from 'react';
import { disabled as off, ring } from '../focus';

export interface ButtonProps {
  variant?: 'primary' | 'secondary' | 'destructive';
  /** `sm` is 28 high, for a row of a panel. */
  size?: 'default' | 'sm';
  /** A toggle: aria-pressed, drawn in the variable chip colours when on. */
  pressed?: boolean;
  disabled?: boolean;
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void;
  /** A name that starts with the visible text, where the text alone does not say what it acts on. */
  'aria-label'?: string;
  /** The id of the text that says why the button is disabled, or what it does beyond its name. */
  'aria-describedby'?: string;
  children: ReactNode;
  [data: `data-${string}`]: string | undefined;
}

const fills = {
  primary: 'bg-accent-strong text-on-accent font-semibold hov:bg-accent-hover act:bg-accent-pressed',
  secondary: 'bg-control text-text font-medium hov:bg-control-selected act:bg-field',
  destructive: 'bg-danger-strong text-on-accent font-semibold hov:bg-danger-hover act:bg-danger-pressed',
};

export function Button({ variant = 'secondary', size = 'default', pressed, disabled = false, onClick, children, ...rest }: ButtonProps) {
  const height = size === 'sm' ? 'h-(--mf-size-control-md) px-3' : 'h-(--mf-size-control-lg) px-4';
  const look = pressed ? 'bg-chip-bg font-semibold text-chip-text' : fills[variant];
  return (
    <button {...rest} type="button" aria-pressed={pressed} disabled={disabled} onClick={onClick} className={`${ring} ${off} ${height} rounded-md text-small ${look}`}>
      {children}
    </button>
  );
}
