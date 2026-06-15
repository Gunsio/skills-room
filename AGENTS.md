# Skillroom Agent Guide

## Product Vision

Skillroom should first be a genuinely useful local-first TUI for managing Agent
Skills: fast search, source switching, local inventory, install/update/remove,
safe actions, and clear details. This baseline matters because a philosophy that
cannot improve daily skill management will stay abstract.

Beyond package management, Skillroom should grow into a product with its own
skill management philosophy. It should help users and teams reason about skill
quality, versioning, collaboration, growth, drift, risk, and retirement. The
long-term goal is not only "which skill can I install", but also "which skill is
good, safe, improving, team-ready, and still worth keeping".

## Product Direction

- Build the local management experience first: transparent sources, reliable
  local scanning, keyboard-first workflows, and Taproom-level TUI polish.
- Treat every remote source as an explicitly configured trust boundary.
- Do not invent capabilities that are not wired. If install/update/remove is not
  available for a source, show that honestly.
- Add governance features only when they answer real skill usage questions:
  quality scorecards, version locks, team promotion, diff/rollback, evals,
  bloat signals, and retirement hints.
- Keep product decisions tied to observable evidence: local state, source
  metadata, action results, test fixtures, usage signals, and review history.

## Open Questions

The living question list is tracked in
[docs/skill-management-questions.md](docs/skill-management-questions.md). Revisit
it periodically and mark which questions Skillroom has answered with product
features, metrics, workflows, or explicit non-goals.

