---
status: accepted
---
# Agents connect through an OAuth grant, never a copied token

An agent connects to a workspace through an OAuth grant given through Gatehouse. No API key, token or pairing token is created, shown or copied by hand. The consent screen lists the risk tiers the agent gets under the current policy; the person approves it once; access tokens are short-lived; revoking the grant ends the agent at once and its next call is refused. A caller without a valid grant is refused, never downgraded.

This replaces one sentence of ADR-0010 ("Agents pair with a random token that is shown once and stored only as a hash") and rule G4 of `docs/standards/modular-thin-ui.md`. The gate, the permit, the risk tiers, the policy per tier and the per-agent overrides in ADR-0010 stand unchanged.

## Why

Decision 10 already makes OAuth the way an MCP client authenticates, and epic E2.2 builds it. A second path that hands a person a secret to paste into an agent's configuration adds a leak risk and a setup step for no gain: the person has to store the secret, rotate it and remember where it went. The owner decided on 16 Nov 2026 that OAuth is the only way (decision 31).

## Considered options

- Pairing token shown once (ADR-0010 as first written): rejected; it is the secret that leaks and the step people skip.
- OAuth and a pairing token side by side: rejected; two ways to connect means two paths to secure, test and document, and the token path is the weaker one.
- OAuth only (chosen).

## Consequences

- On the web, an agent that runs elsewhere gets the MCP endpoint and starts its own OAuth sign-in; the browser shows the consent screen and the agent then appears as an actor.
- On the desktop, the app starts the agent over ACP and obtains one grant for the computer. The app keeps the short-lived token in the system keychain; the agent keeps its own sign-in with its provider, which Meldframe never sees.
- E2.4 is reworded to grants. `docs/prd.md` FR-28 and rule G4 are reworded. NFR-6 (short-lived, revocable session tokens) holds as written.
- The development tokens behind the gate's principal interface in M1 (decision 25) stay a development credential. They are never offered in the product UI and the release build must refuse them (a criterion of E2.4).
- Tests: a revoked grant is refused on the next call; a caller without a grant is refused; the consent lists the tiers of the policy in force.
