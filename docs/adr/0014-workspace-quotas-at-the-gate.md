---
status: accepted
---
# Workspace quotas are decided at the admission gate, one code per limit

The gate (ADR-0010) already decides every write before a room sees it, so it also decides the quotas of a workspace: ops per minute, ops per call, and storage. Each limit has its own stable refusal code, so an agent can tell a wait from a stop.

- `quota_ops_per_minute`: the workspace sent more ops in this minute than its rate allows. Retryable; the reply says when the next minute starts (HTTP 429 with `Retry-After`).
- `batch_too_large`: one call carries more ops than the batch limit (100). Not retryable as sent (HTTP 413). It was `invalid` before; it is a different mistake and needs a different fix.
- `quota_storage`: the workspace wrote as many bytes of ops as it may (the JSON size of each command). Not retryable (HTTP 413).
- `quota_exceeded` (the older total-ops limit) keeps its meaning.

A refused batch charges nothing. The minute is a fixed window of the gate's clock (`now / 60000`), which the caller injects, so tests move time by hand and the core crates never read the system clock.

## Why

Counting where the policy is decided means a refused call costs no store read, every transport (REST, WebSocket, MCP) gets the same answer, and the call log of E2.4 already records the refusal with its code. A fixed window is simple to reason about and to persist (one row per workspace and minute); a sliding window would need the timestamps of recent ops.

## Considered options

- A rate limit in the HTTP adapter only: cheap, but the MCP and WebSocket paths and any future transport would each need it, and it cannot see ops, only requests. Not chosen. (The adapter keeps its own `rate_limited` for too many calls; that is a different limit.)
- Storage as the size of the stored rows: exact, but it needs a read per write. The JSON size of the command is known at the gate and tracks it closely enough for a limit.

## Consequences

- Known limit: the gate charges when it admits a batch, before the room accepts. A batch that stops at a failing op leaves the later ops charged, and a retry is charged again. The window never moves back when the clock does.

- The counters live in the gate, and a workspace starts from what the store holds the first time the gate sees it (`Gate::usage`; with Postgres, `PgStore::workspace_usage` counts the ops and `ops.bytes` of the workspace's own rows, found by the `workspace_id` column of migration 0003; `ops.bytes` is the compact JSON size the gate counts, and rows older than the migration get the size of their stored text, a little larger). So the total and the storage quotas survive a restart. The per-minute count starts empty after a restart: it only ever allows one more minute of ops, and the store has no clock the gate shares.
- `Server::with_quotas` sets the limits and the clock; the defaults are `MAX_OPS_PER_MINUTE` and `MAX_STORAGE_BYTES`.
- The first well-formed call of a workspace asks the store for its totals while the server holds its lock, so that one call is slower and other workspaces wait for it; a lookup that fails is asked again on the next call, and a workspace is never started from a guess.
- Migration 0003 takes a brief lock and an older server would write `files` rows without the new columns: stop writers while it runs (pre-alpha, one server).
