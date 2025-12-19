# ADR 001: WebSockets on actix-ws

## Status

Accepted.

## Date

2025-12-19

## Context

The backend WebSocket adapter relied on `actix-web-actors`, which is now
deprecated.[^deprecation] Continuing to depend on the actor-based API increases
maintenance risk and conflicts with upstream guidance. We also want to keep the
WebSocket adapter aligned with the hexagonal architecture by focusing on
request validation, wire-level framing, and heartbeat management at the
boundary.

## Decision

Adopt `actix-ws` for all WebSocket upgrade handling and message processing in
`backend/src/inbound/ws`.

- Use `actix_ws::handle` to perform the HTTP upgrade and obtain a `Session` and
  `MessageStream`.
- Run an async loop that combines heartbeat ticks with inbound message handling
  via `tokio::select!`.
- Keep domain behaviour unchanged by passing deserialised requests to
  `UserOnboarding` and serialising domain events back to clients.

## Consequences

- `actix-web-actors` and `actix` are removed from the backend dependency graph.
- The WebSocket adapter now uses async/await instead of actor traits while
  preserving origin validation, payload validation, and heartbeat semantics.
- Tests continue to exercise upgrade behaviour and payload responses without
  actor-specific types. Close codes now come from `actix-ws` (via
  `actix_http::ws`).

## Backend design updates

- The WebSocket adapter is documented as a handler-driven inbound adapter that
  uses `actix-ws` for upgrades and message framing.
- The adapter remains responsible for heartbeats and payload translation while
  delegating domain decisions to `UserOnboarding`.

## References

- Issue.[^issue]
- PR.[^pr]
- Review comment.[^comment]

[^deprecation]: <https://rustdocs.webschool.au/actix_web_actors/index.html>
[^issue]:
  <https://github.com/leynos/wildside/issues/246>
[^pr]:
  <https://github.com/leynos/wildside/pull/245>
[^comment]:
  <https://github.com/leynos/wildside/pull/245#discussion_r2594317032>
