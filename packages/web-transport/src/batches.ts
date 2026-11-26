// What the shell does for the Activity panel (E5.1, ADR-0021): it builds the query of the list route. The answers of the
// list and review routes and the `batch` and `touch` frames are not read here: the shell hands the status and the text to the
// app layer (`Action.wire`), which parses and checks them (finding T6 of the thin UI audit).

/** The query of `GET .../batches` for a `fetchBatches` effect; a bound that is null is left out. */
export function listQuery(e: { beforeN: number | null; afterN: number | null; limit: number }): string {
  const q = new URLSearchParams();
  if (e.beforeN !== null) q.set('before', String(e.beforeN));
  if (e.afterN !== null) q.set('after', String(e.afterN));
  q.set('limit', String(e.limit));
  return `?${q}`;
}
