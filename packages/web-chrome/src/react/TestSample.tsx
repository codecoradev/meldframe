// A sample component for the tests of this folder only: no screen uses it. It proves that JSX compiles, that
// props and a selected piece of the snapshot reach the markup, and that the file kind is linted and scanned.
import { type StateSource, useAppState } from './useAppState';

export function TestSample({ label }: { label: string }) {
  return <p data-testid="sample">{label}</p>;
}

/** Shows the mode of the editor, read from the engine's state. */
export function TestMode({ engine }: { engine: StateSource<{ state: { mode: string } }> }) {
  const mode = useAppState(engine, (s) => s.state.mode);
  return <p data-testid="mode">{mode}</p>;
}
