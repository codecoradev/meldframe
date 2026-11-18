---
status: accepted
---
# Assets are images kept on local disk per workspace, content-addressed, with the type told by their bytes

E2.5 needs images for image fills. This ADR records the choices of the first slice (the `assets` feature crate) and the answers of the owner for the next slices.

Decided in the first slice:

- An asset is png, jpeg, webp or gif, told by its first bytes. SVG is refused (`asset_type_refused`): it can carry script and external references, and an asset is served back to a browser.
- An asset is named by the lowercase hex SHA-256 of its bytes, the form `image.assetId` already takes. The same bytes are the same asset; storing them again writes nothing unless the stored file no longer matches.
- Assets belong to one workspace (ADR-0006, E2.3): the store keys them by workspace and id, so the same image in two workspaces is kept twice and one workspace never reads another's asset, even with its id (`asset_not_found`).
- `BlobStore` is the seam; `DiskBlobs` keeps `<root>/<SHA-256 of the workspace>/<id[..2]>/<id>` (a fixed-length directory for any workspace name), written to a temporary name, flushed and renamed (and the directory flushed), and checks the id again when it reads: a file changed on disk or cut short is `asset_corrupt`, never served under the old id, and uploading the same image again writes it anew. An upload is at most 10 MiB, checked before it is hashed. `DiskBlobs` is not built for wasm32.

## Decided by the owner (the next slices)

The owner accepted every proposal on 18 Nov 2026:

1. Upload route: a raw body `POST /v1/assets` with the image bytes; no multipart parser. It answers `{"ok": true, "id", "type", "bytes"}`.
2. How the browser fetches an asset: `GET /v1/assets/{id}` with the Bearer token (through the dev proxy now, with the Gatehouse session later). It is served with its sniffed `image/*` type, `X-Content-Type-Options: nosniff` and a CSP that allows no script (only the first bytes are checked), `Cache-Control: private, max-age=31536000, immutable` (the id is the content, and the asset is private to the workspace).
3. An agent tool to upload: base64 in a JSON call, which the MCP body limit bounds.
4. A storage quota for assets: counted with `quota_storage` of the workspace, not a limit of its own.
5. The root directory is `MELDFRAME_ASSETS_DIR`, required in a release build (a development build keeps assets in `.dev-assets`).

What is built: the routes (1, 2), the agent tool (3), the quota (4) and the root directory (5). The quota: `assets::Usage` keeps the bytes of the assets of each workspace (it starts from what the store holds, so a restart loses nothing); an upload of an image the workspace does not hold yet reserves its bytes against the storage limit together with the bytes of its ops (the reservation goes back when the write fails or finds the image already written by another call, so the same image sent twice at once is charged once) and is refused with `quota_storage` when it does not fit (a store that cannot say what the workspace holds is `unavailable`, retryable), and the gate counts the same asset bytes when it admits an op (`Gate::extra_storage`). An image the workspace holds already costs nothing. The agent tool (3) is built: `upload_asset` (Change tier, base64 in a JSON call, so an agent uploads what fits an MCP request, and sends a larger image to `POST /v1/assets`). The canvas that renders image fills is the last slice.

## Considered options

- Assets in Postgres (`bytea`): one store to back up, but large rows in the database that every backup copies; the PRD names local disk behind a blob interface.
- A global content-addressed store shared by every workspace: saves disk, but whether a workspace may read an asset becomes an index lookup that a bug can skip; per-workspace paths make the tenancy rule hold by construction.

## Consequences

- The disk is part of what must be backed up with the database; a file removed from disk makes the fills that name it render without their image (`asset_not_found` once the transport exists).
- Unused assets are never deleted yet (no reference count), and a `.tmp` file a crash left between the write and the rename is not removed yet.
