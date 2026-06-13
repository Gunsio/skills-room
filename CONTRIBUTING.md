# Contributing

Skillroom is being built milestone by milestone. Keep changes aligned with the
current milestone acceptance criteria in the design document.

## Development Rules

- Keep domain logic independent from UI and command execution.
- Add or update tests with each behavioral change.
- Do not add runtime dependencies without documenting the purpose, alternative,
  license, and CI impact.
- Do not run destructive skill operations against a real HOME during tests.

## Local Checks

The full CI scripts are introduced in M0. Until then, run:

```bash
cargo check
```
