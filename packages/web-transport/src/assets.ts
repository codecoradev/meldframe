// What the shell does for the image picker (E2.5, ADR-0015 follow-up): the query of `GET /v1/assets` for a `fetchAssets`
// effect. The answer is not read here: the shell hands the status and the text to the app layer (`Action.wire`), which
// parses and checks it (finding T6). There is no upload here: before sign-in an agent uploads through `upload_asset`, and
// the picker lists what the workspace holds.

/** The route's page size is 1 to 200. */
const MAX_PAGE = 200;
/** The lowercase hex SHA-256 the server names an asset by. */
const ID = /^[0-9a-f]{64}$/;

/** The query of `GET /v1/assets` for a `fetchAssets` effect: the limit held to 1 to 200, and a cursor only when it is an asset id. */
export function assetsQuery(e: { after: string | null; limit: number }): string {
  const q = new URLSearchParams();
  q.set('limit', String(Math.min(MAX_PAGE, Math.max(1, Math.trunc(e.limit) || 1))));
  if (e.after !== null && ID.test(e.after)) q.set('after', e.after);
  return `?${q}`;
}
