---
status: accepted
---
# Workspace quotas are decided at the admission gate, one code per limit

The gate (ADR-0010) already decides every write before a room sees it, so it also decides the quotas of a workspace: ops per minute, ops per call, and storage. Each limit has its own stable refusal code, so an agent can tell a wait from a stop.

- `quota_ops_per_minute`: the workspace sent more ops in this minute than its rate allows. Retryable; the reply says when the next minute starts (HTTP 429 with `Retry-After`).
- `batch_too_large`: one call carries more ops than the batch limit (100), or more than the ops per minute of the workspace (no minute can hold it; a rate of 0 refuses every batch as a wait instead). Not retryable as sent (HTTP 413). It was `invalid` before; it is a different mistake and needs a different fix.
- `quota_storage`: the workspace wrote as many bytes of ops as it may (the JSON size of each command). Not retryable (HTTP 413).
- `quota_exceeded` (the older total-ops limit) keeps its meaning.

A refused batch charges nothing. The minute is a fixed window of the gate's clock (`now / 60000`), which the caller injects, so tests move time by hand and the core crates never read the system clock.

## Why

Counting where the policy is decided means every transport (REST, WebSocket, MCP) gets the same answer, and the call log of E2.4 already records the refusal with its code. A fixed window is simple to reason about and to persist (one row per workspace and minute); a sliding window would need the timestamps of recent ops.

## Considered options

- A rate limit in the HTTP adapter only: cheap, but the MCP and WebSocket paths and any future transport would each need it, and it cannot see ops, only requests. Not chosen. (The adapter keeps its own `rate_limited` for too many calls; that is a different limit.)
- Storage as the size of the stored rows: exact, but it needs a read per write. The JSON size of the command is known at the gate and tracks it closely enough for a limit.

## Consequences

- Known limit: the gate charges when it admits a batch, before the room accepts. A batch that stops at a failing op leaves the later ops charged, and a retry is charged again. The window never moves back when the clock does: after the server clock steps back, a workspace stays in the newer window it counted until the clock reaches it again.
- A batch larger than the ops per minute can never pass, so it is `batch_too_large` with that limit, not a retryable `quota_ops_per_minute`.
- Storage counts the op log, which only grows: a remove does not give room back and is itself refused once the storage is used up.
- The gate runs after the server opened the room of the file (a closed file is restored from the store) and looked up the keys of the batch for idempotent replays, so a refused call is not free of store reads; the checks themselves read nothing.

- The counters live in the gate, and a workspace starts from what the store holds the first time the gate sees it (`Gate::usage`; with Postgres, `PgStore::workspace_usage` counts the ops and `ops.bytes` of the workspace's own rows, found by the `workspace_id` column of migration 0003; `ops.bytes` is the compact JSON size the gate counts, and rows older than the migration get the size of their stored text, a little larger). So the total and the storage quotas survive a restart. The per-minute count does too: at the same first sight the gate asks for the ops the workspace wrote since the start of its current minute (`Gate::recent`; with Postgres, `PgStore::ops_since` on `ops.created_at`), and keeps the larger of that and what it counted itself (or a newer window it already counted in, when the clock stepped back). The two lookups are asked for apart, so a failed one does not lose the other, and both run under the server lock on that first call (migration 0004 indexes `ops (file_id, created_at)` for the minute). The minute needs the gate's clock in milliseconds since the Unix epoch. The store's time is the database clock and the gate's is the server's, so a skew between them moves the window edge by that much; ops the gate admitted but the room refused are counted by the gate and not by the store.
- `Server::with_quotas` sets the limits and the clock; the defaults are `MAX_OPS_PER_MINUTE` and `MAX_STORAGE_BYTES`.
- The first well-formed call of a workspace asks the store for its totals (and its minute) while the server holds its lock, so that one call is slower and other workspaces wait for it; a lookup that fails is asked again on the next call, and a workspace is never started from a guess.
- Migration 0003 takes a brief lock and an older server would write `files` rows without the new columns: stop writers while it runs (pre-alpha, one server).
