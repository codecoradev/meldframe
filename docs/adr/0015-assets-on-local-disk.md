---
status: proposed
---
# Assets are images kept on local disk per workspace, content-addressed, with the type told by their bytes

E2.5 needs images for image fills. This ADR records the choices of the first slice (the `assets` feature crate) and names the ones the next slices still need from the owner.

Decided in the first slice:

- An asset is png, jpeg, webp or gif, told by its first bytes. SVG is refused (`asset_type_refused`): it can carry script and external references, and an asset is served back to a browser.
- An asset is named by the lowercase hex SHA-256 of its bytes, the form `image.assetId` already takes. The same bytes are the same asset; storing them again writes nothing unless the stored file no longer matches.
- Assets belong to one workspace (ADR-0006, E2.3): the store keys them by workspace and id, so the same image in two workspaces is kept twice and one workspace never reads another's asset, even with its id (`asset_not_found`).
- `BlobStore` is the seam; `DiskBlobs` keeps `<root>/<SHA-256 of the workspace>/<id[..2]>/<id>` (a fixed-length directory for any workspace name), written to a temporary name, flushed and renamed (and the directory flushed), and checks the id again when it reads: a file changed on disk or cut short is `asset_corrupt`, never served under the old id, and uploading the same image again writes it anew. An upload is at most 10 MiB, checked before it is hashed. `DiskBlobs` is not built for wasm32.

## Open for the owner (the next slices)

1. Upload route: a raw body `POST /v1/assets` with the image bytes (proposed; no multipart parser), or multipart.
2. How the browser fetches an asset (whatever the route, it is served with its sniffed `image/*` type, `X-Content-Type-Options: nosniff` and a CSP that allows no script, since only the first bytes are checked): `GET /v1/assets/{id}` with the Bearer token through the dev proxy now, and with the Gatehouse session later (proposed), or signed URLs.
3. An agent tool to upload (base64 in a JSON call, which the MCP body limit bounds) or uploads by people only.
4. A storage quota for assets: counted with `quota_storage` of the workspace (proposed) or a limit of its own.
5. The root directory: `MELDFRAME_ASSETS_DIR`, required in a release build (proposed).

## Considered options

- Assets in Postgres (`bytea`): one store to back up, but large rows in the database that every backup copies; the PRD names local disk behind a blob interface.
- A global content-addressed store shared by every workspace: saves disk, but whether a workspace may read an asset becomes an index lookup that a bug can skip; per-workspace paths make the tenancy rule hold by construction.

## Consequences

- The disk is part of what must be backed up with the database; a file removed from disk makes the fills that name it render without their image (`asset_not_found` once the transport exists).
- Unused assets are never deleted yet (no reference count), and a `.tmp` file a crash left between the write and the rename is not removed yet.
