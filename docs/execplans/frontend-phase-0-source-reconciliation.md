# Reconcile front-end source authority

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

This phase makes the Wildside front-end documentation implementable before
feature work starts. After completion, contributors can tell which document
owns each platform, product, schema, styling, localization, accessibility, and
testing requirement, and roadmap tasks can cite those sources instead of
becoming the source of design decisions.

## Constraints

`docs/frontend-roadmap.md` remains the task catalogue, not the primary design
authority. Platform guidance in `docs/v2a-front-end-stack.md` takes precedence
over older Progressive Web Application (PWA) design material when the documents
conflict. Substantive policy, schema, and contract decisions must move into a
design document or Architecture Decision Record (ADR). Imported v2a lint and
token work must cite the source repository and local ownership path.

## Tolerances

Escalate if reconciliation requires changing backend contracts, removing an
existing documented product requirement, or choosing between incompatible token,
state, cache, or localization policies. Escalate if the mockup source cannot be
accessed or if imported lint rules require dependencies not approved by the
stack decision.

## Risks

The main risk is moving requirements into the roadmap instead of their owning
documents. Mitigate this by treating every roadmap edit as a citation refresh
unless the change is only task sizing, dependency ordering, or acceptance
criteria.

## Plan

Start with `docs/frontend-roadmap.md` phase 0 and create an authority catalogue
covering `docs/v2a-front-end-stack.md`, `docs/wildside-pwa-design.md`,
`docs/wildside-pwa-data-model.md`, `docs/wildside-ux-state-graph-v0.1.json`,
`docs/sitemap.md`, `spec/openapi.json`, and `spec/asyncapi.yaml`. Record which
document owns each topic and which topics need a design document or ADR update.

Next, reconcile the PWA design material against the v2a stack direction. Import
and document the v2a localization, accessibility, semantic CSS, testing, and
architectural lint gates. Import the latest token and design-system source into
the repository-owned `packages/tokens/` pipeline without committing generated
mockup artefacts as source.

Finish by replacing decision prose in roadmap phases 1-5 with inline citations
to the reconciled source documents and by updating task dependencies where a new
ADR, schema update, or design-document merge gates implementation.

## Validation

Run these commands from the repository root:

```bash
make fmt
make markdownlint
make nixie
```

Success means the source catalogue, reconciled design documents, imported lint
documentation, token pipeline notes, and roadmap citations are all present and
Markdown and Mermaid validation pass.

## Progress

- [x] Draft phase-level ExecPlan.
- [ ] Catalogue authoritative sources and contradictions.
- [ ] Reconcile source documents and ADR needs.
- [ ] Import and document v2a lint and token requirements.
- [ ] Refresh roadmap citations and dependencies.

## Surprises & Discoveries

None yet.

## Decision Log

- 2026-04-28: Keep this plan at phase granularity and keep task detail in
  `docs/frontend-roadmap.md` to avoid duplicating the roadmap.

## Outcomes & Retrospective

Not started.
