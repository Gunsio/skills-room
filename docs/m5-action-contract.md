# M5 Action Safety Contract

M5 adds write operations to the TUI. The implementation must keep every write
operation explainable, typed-confirmed, non-shell executed, lock-protected, and
recoverable.

## Confirmed Scope

- Install selected skill.
- Update selected skill.
- Update all skills that are explicitly known to be updateable.
- Remove selected skill.
- Open selected skill path.
- Copy selected skill path.

## D5 Removal Semantics

Implementation scope for M5:

- Remove targets the selected skill at its selected path and scope.
- Remove does not support per-agent removal in M5.
- The confirmation must show skill name, path, scope, enabled agents, impact,
  full argv, and require `REMOVE`.
- Removal is blocked when the command would touch a real home path while running
  under test mode.

Reasoning:

- The current inventory model treats a skill path as the smallest reliable write
  unit.
- Per-agent removal needs an agent-to-skill ownership contract that does not
  exist yet.

## D7 Version And Update Semantics

Implementation scope for M5:

- `Update selected` is available when the selected record has an update command.
- `Update all` includes only skills whose state is `UpdateAvailable` and whose
  update command can be converted to a structured argv.
- Unknown version or unknown source records are excluded from no-confirm batch
  update and must produce a readable reason.

Reasoning:

- M4 source metadata already carries enough information to distinguish known
  updateable records from unknown records.
- Adding a new external CLI JSON contract would expand M5 beyond the safe-action
  milestone.

## Command Execution

- Every executable operation is represented as an argv array.
- The runner must call the executable directly with args; shell execution is not
  allowed.
- stdout and stderr are streamed into the output panel.
- The final status records exit code, argv, source, and reason.
- A skill-level execution lock prevents duplicate install, update, or remove for
  the same selected skill.

## Test Guard

- Unit and integration tests must never delete a real home skill.
- Destructive remove tests must use a temp home, fixture path, or mock runner.
- A real-home remove plan must be rejected before execution when test mode is
  active.
