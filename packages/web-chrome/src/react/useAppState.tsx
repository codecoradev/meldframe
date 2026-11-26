// The one way a React view reads the app layer: a subscription over the engine's snapshot, never a copy of it.
// The file is .tsx although it holds no markup: every file that imports React is .tsx, so that the hook lint,
// the literal scan and the boundary check, which all go by extension, see it.
import { useCallback, useMemo, useSyncExternalStore } from 'react';

/**
 * The part of the engine (`Engine` of @meldframe/web-link) that a view needs. It is declared here, not imported, so
 * that this kit depends on no other package: the engine fits it as it is.
 */
export interface StateSource<S> {
  /** The current snapshot. It must be the same object for as long as nothing changed (the engine guarantees it). */
  snapshot(): S;
  /** Calls `fn` after each change; returns the function that stops it. */
  subscribe(fn: (snapshot: S) => void): () => void;
}

/**
 * Reads the engine into a component and draws it again when what it selected changed.
 *
 * Stable-reference rule: `select` must give the same value (`Object.is`, or `isEqual` when you pass one) for the same
 * part of the snapshot. Pick a field (`(s) => s.state.mode`), or pass `isEqual` for a derived value. A selector that
 * builds a new object or array on every call and has no `isEqual` draws the component again after every change of any
 * part of the snapshot, and React warns "The result of getSnapshot should be cached". Without `select` the whole
 * snapshot is returned, which is already stable.
 *
 * `select` and `isEqual` may be inline functions: a value is cached for the snapshot it came from, and only a source
 * that is another object makes the hook subscribe again.
 */
export function useAppState<S>(source: StateSource<S>): S;
export function useAppState<S, T>(source: StateSource<S>, select: (snapshot: S) => T, isEqual?: (a: T, b: T) => boolean): T;
export function useAppState<S, T>(source: StateSource<S>, select?: (snapshot: S) => T, isEqual: (a: T, b: T) => boolean = Object.is): S | T {
  // One subscription per source: React subscribes again whenever this function is another one.
  const subscribe = useCallback((notify: () => void) => source.subscribe(() => notify()), [source]);
  const read = useMemo(() => selection(source, select, isEqual), [source, select, isEqual]);
  return useSyncExternalStore(subscribe, read, read);
}

/** A getSnapshot for React: calls `select` once per snapshot and keeps the earlier value when `isEqual` says so. */
function selection<S, T>(source: StateSource<S>, select: ((s: S) => T) | undefined, isEqual: (a: T, b: T) => boolean): () => S | T {
  if (!select) return () => source.snapshot();
  let from: S | undefined;
  let value: T | undefined;
  let known = false;
  return () => {
    const snapshot = source.snapshot();
    if (known && Object.is(snapshot, from)) return value as T;
    const next = select(snapshot);
    if (!(known && isEqual(value as T, next))) value = next;
    from = snapshot;
    known = true;
    return value as T;
  };
}
