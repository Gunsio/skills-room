# Governance

## M0 Local Repository Decision

Skillroom is developed locally during the milestone execution phase.

- Remote: not configured in M0.
- Push policy: do not push to any remote unless explicitly requested.
- Default branch: `main`.
- Milestone branches: one branch per milestone, named `milestone/<id>-<topic>`.
- Commit policy: one commit per milestone Todo item.
- Merge policy: merge a milestone branch into `main` only after all milestone
  acceptance checks pass.

## Internal Open Source Governance

The final repository owner, reviewer group, and remote hosting location are
deferred to the internal open source preparation milestone. Until then, local
work is reviewed by the user at milestone boundaries.
