# ADR 003: Protected route tile delivery

## Status

Proposed. Implementation is tracked by backend roadmap section 6.1.

## Date

2026-10-19

## Context and problem statement

Martin must serve public points of interest (POIs) and private route tiles from
backend-owned data. The earlier architecture sketch put JSON Web Token (JWT)
validation in `pgjwt` and supplied a route-ID-first SQL signature. Martin's
function interface takes tile coordinates followed by optional query JSON; an
HTTP Authorization header is not automatically a SQL argument. Read-only tile
credentials also do not establish end-user authorization.

## Decision drivers

- Keep access policy in the backend and delivery in adapters.
- Deny requests before retrieving private bytes or consulting shared caches.
- Keep the engine focused on ingestion and route inference.
- Match the existing chart's standard Kubernetes Ingress interface.

## Requirements

The design must bind each route grant to a subject, route revision, audience,
issuer, and expiry, recheck current access, and prevent direct Martin bypass.
The database role must have no application write or schema-management rights.
Domain services must publish events and metrics through their ports only.

## Options considered

### Option A: Validate JWTs inside the tile function

This requires explicit credential transport into SQL, database verification
keys and extension support, and careful handling of token-bearing query strings
and caches. It also duplicates application access policy and complicates
rotation under the read-only role. The previous sketch did not specify those
contracts.

### Option B: Validate and proxy in the backend adapter

An inbound adapter validates bearer JWTs and invokes backend access policy. An
outbound adapter then constructs a fixed internal Martin request using
server-derived route/revision parameters. SQL performs only read-only geometry
selection and MVT encoding. This adds a backend network hop and requires
bounded proxy resources, but provides one enforceable authorization and
revocation path.

## Decision outcome / proposed direction

Choose option B. Public POI tiles may reach Martin through an allowlisted
Ingress path. Route tiles must pass through the backend validator; never expose
Martin's internal route source or catalogue publicly. Disable shared caching of
private responses initially. Verify tokens and current access on every request,
including cache hits. Keep JWT keys outside PostgreSQL and Martin configuration.

The [tile delivery design](backend-tile-delivery-design.md) defines public and
internal URLs, claims, migration order, role grants, cache rules, network
restrictions, rollout tests, and the proposed engine roadmap follow-up.

## Known risks and limitations

The backend hop adds latency and capacity demand; measure it separately from
Martin latency. Ingress rewriting varies by controller and must be tested in
both deployment modes. Network policy alone cannot filter source paths from an
allowed ingress controller. Walkable route geometry is an upstream engine gap,
so private route enablement remains blocked until that contract is met.

## Outstanding decisions

Implementation must select and pin a Martin image, verify its health and
metrics interfaces, and confirm controller-specific path rewriting. Those
selections must preserve the request and authorization contracts above.
