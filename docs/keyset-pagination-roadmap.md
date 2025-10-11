# Roadmap: keyset pagination crate for Wildside backend

## Phase 1 – Establish pagination core

### Step 1.1 – Define cursor primitives and encoding
- **Tasks**
  - [ ] Implement `Direction` enum and `Cursor<K>` struct covering forward and
        backward traversal semantics.
  - [ ] Provide base64url JSON encoding and decoding helpers with explicit
        error handling across representative key payloads.
- **Testing**
  - [ ] Unit: Round-trip encode and decode sample payloads, inject malformed
        inputs, and assert precise error variants.
  - [ ] Behavioural: Exercise a thin Actix endpoint that echoes cursors through
        request and response cycles to prove stable token handling.
- **Outcome**
  - Library consumers can construct, encode, and decode opaque cursor tokens
    safely, enabling consistent pagination state transfer.
- **Dependencies**
  - None.

### Step 1.2 – Model pagination keys and traits
- **Tasks**
  - [ ] Define marker traits or helper utilities that bind endpoint-specific key
        structs to cursor generation while keeping the crate generic.
  - [ ] Supply example implementations for composite keys, including
        `(created_at, id)` for users, highlighting index expectations in
        documentation.
- **Testing**
  - [ ] Unit: Validate trait blanket implementations and ensure compile-time
        guarantees for required ordering fields.
  - [ ] Behavioural: Demonstrate trait usage inside a sample handler pipeline
        that constructs cursors for multiple key types.
- **Outcome**
  - Endpoints can declare stable ordering keys with clear integration guidance
    and compiler enforcement.
- **Dependencies**
  - Completion of Step 1.1 to reuse cursor types.

### Step 1.3 – Craft response envelope types
- **Tasks**
  - [ ] Implement `PaginationLinks` and `Paginated<T>` structs matching the
        design envelope, including `self_`, `next`, and `prev` link fields.
  - [ ] Derive serialization and OpenAPI schema traits, ensuring optional links
        are omitted when unavailable and the `limit` guardrail is documented.
- **Testing**
  - [ ] Unit: Snapshot serialize representative payloads and assert link
        omission rules under varying cursor availability.
  - [ ] Behavioural: Exercise generated envelopes through a mock Actix response
        to verify OpenAPI documentation and runtime fields align.
- **Outcome**
  - Shared response container guarantees consistent pagination metadata across
    endpoints.
- **Dependencies**
  - Step 1.1 for cursor reuse in link composition tests.

## Phase 2 – Request handling utilities

### Step 2.1 – Build query parameter extractor
- **Tasks**
  - [ ] Introduce `PageParams` with optional `cursor` and `limit` values, plus
        helper methods enforcing defaults, maximums, and explicit validation
        errors.
  - [ ] Document parser failure modes so handlers surface actionable guidance to
        clients.
- **Testing**
  - [ ] Unit: Cover parsing edge cases, including missing limits, non-numeric
        values, oversized requests, and double cursor submission.
  - [ ] Behavioural: Drive the extractor through Actix integration tests to
        confirm HTTP 400 responses and error payload structure.
- **Outcome**
  - Actix handlers receive validated pagination inputs with predictable
    defaults.
- **Dependencies**
  - Step 1.1 to decode cursors during validation.

### Step 2.2 – Provide pagination context helpers
- **Tasks**
  - [ ] Offer builder or utility functions that encapsulate limit selection,
        directional intent, and cursor decoding to reduce boilerplate inside
        handlers.
  - [ ] Document expected handler workflow, including how to compute boundaries
        and propagate cursors into link URLs.
- **Testing**
  - [ ] Unit: Validate helper logic for forward and backward navigation,
        including limit coercion and cursor canonicalization.
  - [ ] Behavioural: Exercise helpers inside a mock handler to confirm the
        integration sequence and error propagation.
- **Outcome**
  - Crate consumers follow a consistent integration sequence with minimal
    repeated logic.
- **Dependencies**
  - Steps 1.1 and 2.1 for cursor and parameter primitives.

## Phase 3 – Database integration patterns

### Step 3.1 – Implement Diesel query adapters
- **Tasks**
  - [ ] Supply helper functions or macros to apply lexicographic filters for
        `Next` and `Prev` cursors against Diesel query builders, targeting
        composite key comparisons.
  - [ ] Provide guidance for both synchronous and `diesel_async` contexts with
        example queries that fetch `limit + 1` results.
- **Testing**
  - [ ] Unit: Validate generated predicates for single and multi-column keys,
        including SQL rendering assertions.
  - [ ] Behavioural: Run Diesel-backed integration tests that paginate over
        seeded datasets in both synchronous and async executors.
- **Outcome**
  - Query construction becomes predictable and less error-prone when applying
    cursor-based filtering.
- **Dependencies**
  - Steps 1.1, 1.2, and 2.2 to understand key semantics and handler context.

### Step 3.2 – Package boundary evaluation utilities
- **Tasks**
  - [ ] Implement helper routines that accept fetched records, trim the extra
        item, determine presence of additional pages, and generate
        direction-appropriate cursors.
  - [ ] Support both forward and backward navigation paths with deterministic
        handling of off-by-one and empty-page scenarios.
- **Testing**
  - [ ] Unit: Cover boundary trimming, cursor derivation, and empty result sets
        to guard against regressions.
  - [ ] Behavioural: Validate helpers end-to-end by wiring them to Diesel query
        adapters inside scenario-driven integration tests.
- **Outcome**
  - Handlers can transform raw query results into paginated responses without
    duplicating edge-case logic.
- **Dependencies**
  - Steps 1.1 through 3.1 for cursor structures and query patterns.

## Phase 4 – Actix-web integration and ergonomics

### Step 4.1 – Compose HTTP response builders
- **Tasks**
  - [ ] Provide utility functions that assemble `Paginated<T>` responses,
        including URL construction for `self`, `next`, and `prev` links while
        explicit `limit` selections.
  - [ ] Offer extension traits or helper methods to combine request context,
        cursor strings, and route metadata.
- **Testing**
  - [ ] Unit: Assert link formatting, limit propagation, and omission of
        optional links under boundary conditions.
  - [ ] Behavioural: Run Actix response tests verifying generated envelopes
        integrate with router metadata and produce valid JSON.
- **Outcome**
  - Actix endpoints can emit complete hypermedia envelopes with minimal
    boilerplate and consistent link formatting.
- **Dependencies**
  - Steps 1.3, 2.2, and 3.2 for link metadata and boundary evaluation.

### Step 4.2 – Update `/api/users` handler as reference implementation
- **Tasks**
  - [ ] Refactor the existing users listing endpoint to consume the crate
        utilities end-to-end, ensuring compatibility with async Diesel queries
        and existing response schema expectations.
  - [ ] Capture integration tests validating cursor navigation across forward
        and backward flows, including limit enforcement.
- **Testing**
  - [ ] Unit: Cover handler-specific fallbacks, including default limits and
        empty dataset responses using lightweight state doubles.
  - [ ] Behavioural: Execute end-to-end HTTP tests demonstrating cursor cycling
        and verifying regression coverage for prior bugs.
- **Outcome**
  - Demonstrated end-to-end adoption validates the crate and serves as a living
    example for other endpoints.
- **Dependencies**
  - Completion of Phases 1 through 3 to supply the supporting APIs.

## Phase 5 – Quality, tooling, and documentation

### Step 5.1 – Harden error handling and observability
- **Tasks**
  - [ ] Standardize error types returned by the crate, mapping decoding,
        validation, and database misuse issues to actionable messages.
  - [ ] Emit structured logs or tracing spans for pagination failures, ensuring
        privacy by excluding cursor contents.
- **Testing**
  - [ ] Unit: Assert each error case maps to the correct variant and telemetry
        payload without leaking sensitive cursor data.
  - [ ] Behavioural: Reproduce historical failure scenarios in integration tests
        and demonstrate log or span output captures the fix.
- **Outcome**
  - Operational visibility improves while keeping cursor tokens opaque and
    secure.
- **Dependencies**
  - Steps 1.1 through 4.1 for context around error surfaces.

### Step 5.2 – Author documentation and migration guidance
- **Tasks**
  - [ ] Produce crate-level README and API docs summarizing configuration,
        usage examples, and extension points for other models (POIs, routes,
        etc.).
  - [ ] Draft migration checklist for future endpoints, covering index
        requirements and testing expectations.
- **Testing**
  - [ ] Unit: Run `cargo doc` and markdown linting to ensure examples compile
        and documentation references resolve.
  - [ ] Behavioural: Conduct a dry-run onboarding workshop with the `/api/users`
        team and capture follow-up fixes from their feedback.
- **Outcome**
  - Teams adopting the crate have authoritative guidance and know how to verify
    their integrations.
- **Dependencies**
  - Completion of functional work to document accurately.

### Step 5.3 – Finalize release readiness
- **Tasks**
  - [ ] Ensure lint, format, and test automation covers the crate via Makefile
        targets, updating CI configurations if required.
  - [ ] Add changelog entry describing the new pagination capabilities and their
        scope limitations (no total counts, opaque cursors only).
- **Testing**
  - [ ] Unit: Execute crate-specific lint, format, and test targets locally to
        prove automation coverage.
  - [ ] Behavioural: Run the CI pipeline in a staging environment and confirm it
        gates merges on the new pagination checks.
- **Outcome**
  - Pagination crate is production-ready with tooling support and transparent
    release notes.
- **Dependencies**
  - Prior roadmap steps to deliver the implementation and tests.
