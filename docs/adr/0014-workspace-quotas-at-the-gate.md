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

- The counters live in the gate, in memory. They survive a restart only when the next slice persists them (a workspace column on the stored tables and a counters table); until then a restart starts the counts again from zero, and the epic box stays open.
- `Server::with_quotas` sets the limits and the clock; the defaults are `MAX_OPS_PER_MINUTE` and `MAX_STORAGE_BYTES`.
