---
status: accepted
---
# Workspace is the tenant; authentication through Gatehouse

People and agents authenticate through Gatehouse (OIDC for people, OAuth for MCP clients). A workspace is the tenant: files, variables, assets and quotas belong to one workspace and no data is reachable without the workspace in the principal. Quotas are per workspace. Assets are stored on local disk first, behind a blob interface that can gain an object-storage adapter later.

## Considered options

- Own user store and session system: rejected; Gatehouse already serves the other CodeCora products.
- Per-user quotas only (as in the spike): replaced; the spike keyed quotas by actor and kept them in memory.

## Consequences

The spike's static tokens and in-memory quotas are replaced. The MCP OAuth flow through Gatehouse is not yet proven and is an early W1 task.
