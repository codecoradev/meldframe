---
status: accepted
---
# One admission gate in front of every change

Every change from every transport (WebSocket, MCP, REST, editor) passes one admission step that decides authorisation, mode, quota, timeout and error mapping. The room accepts a write only with a permit that only the gate can create, so a path that skips the gate does not compile. Calls have risk tiers (Read, Change, Delete) with policy per tier and optional per-agent overrides; deleted data goes to the trash first. Agents pair with a random token that is shown once and stored only as a hash. Policy that cannot be read means reads only.

## Mapping to live apply

Working default (13 Nov 2026, decision 26), open to the owner's override: Read is Auto; Change is Auto (applied live, reviewed afterwards); Delete of a frame or of a subtree above a size threshold is Ask for an agent session, Auto below it, and always revertible. See the questionnaire, section 4.

## Considered options

- Policy inside each transport (what the spike did: WebSocket, MCP and a third partial copy): rejected; the copies drifted, and the MCP default was not atomic although the principle says it should be.

## Consequences

The first refactor of the seed code is to consolidate admission before adding Gatehouse and workspace tenancy. Workspace identity comes from the principal the gate receives.
