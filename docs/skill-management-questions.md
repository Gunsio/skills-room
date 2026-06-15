# Skill Management Questions

This file tracks the product-level questions Skillroom should answer over time.
The list is intentionally framed as open questions, not settled doctrine. Review
it after each major milestone and mark whether the product has answered the
question with concrete UI, data, workflow, or documentation.

## P0: Core Product Questions

- [ ] How do we judge whether a skill is good?
  - A useful answer likely needs more than downloads or stars: trigger accuracy,
    task success rate, failure modes, maintenance quality, risk, cost, and
    repeated usage should all matter.
- [ ] How do we decide whether a skill update is positive or negative?
  - A useful answer likely needs golden tasks, regression tasks, trigger tests,
    diff risk, and visible before/after behavior.
- [ ] What is the best practice when multiple people use the same skill in one
  project?
  - A useful answer likely needs project-level pinning, a lockfile, local
    override support, team promotion, rollback, and clear separation between
    personal, project, and organization channels.
- [ ] Should teams update shared skills together, or let each person fork and
  drift?
  - A useful answer likely needs explicit policies: canary, stable, project
    overlay, fork, promote, deprecate, and rollback.
- [ ] When does a skill become too large, overfit, or worse because of too many
  "improvements"?
  - A useful answer likely needs bloat indicators: instruction size, dependency
    count, trigger breadth, exception count, unused sections, and eval
    regression after growth.

## P1: Governance And Lifecycle Questions

- [ ] What is the right boundary of a skill?
  - Is it a knowledge note, operation manual, tool wrapper, workflow protocol, or
    agent behavior constraint? When should one skill be split into several?
- [ ] How should multiple skills compose when more than one applies?
  - A useful answer needs priority, conflict handling, chaining rules, and
    visible reasoning for why one skill was selected.
- [ ] What trust model should Skillroom apply to each source?
  - Internal marketplace, official curated list, local filesystem, personal Git
    repo, and arbitrary URL should not have the same risk posture.
- [ ] How do we know whether a skill is project-specific, organization-ready, or
  public-open-source-ready?
  - A useful answer needs portability signals: hidden assumptions, internal
    dependencies, sensitive references, installability, docs, and tests.
- [ ] How should stale or replaced skills be retired?
  - A useful answer needs unused-skill detection, replacement suggestions,
    deprecation metadata, and safe removal workflows.

## P2: Product Experience Questions

- [ ] What should the user see before installing or updating a skill?
  - A useful answer needs source, trust level, scripts, permissions, dependency
    changes, version delta, and expected local writes.
- [ ] How should Skillroom explain why a skill is recommended for a need?
  - A useful answer needs search relevance, tags, examples, usage history,
    source quality, and compatibility with current project context.
- [ ] What should a skill quality scorecard look like without becoming fake
  precision?
  - A useful answer should show evidence and dimensions, not one opaque score.
- [ ] How should Skillroom support experimentation without polluting team state?
  - A useful answer needs sandbox installs, personal channels, temporary
    overrides, and clean promotion paths.
- [ ] What should be considered a non-goal?
  - A useful answer should prevent Skillroom from becoming a generic package
    manager, a marketplace clone, or an unbounded knowledge-base browser.

