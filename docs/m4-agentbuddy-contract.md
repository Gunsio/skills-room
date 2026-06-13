# M4 AgentBuddy Source Contract

This contract records only facts verified during M4 implementation plus the
local assumptions needed to make the source manager testable.

## Verified Facts

- AgentBuddy npm package: `agentbuddy`.
- Verified package version from `https://bnpm.byted.org`: `0.4.0`.
- Package binary: `agentbuddy`.
- Node engine from package metadata: `>=18`.
- CLI commands documented by the package README:
  - `agentbuddy login`
  - `agentbuddy logout`
  - `agentbuddy get-jwt`
  - `agentbuddy set-jwt <token>`
  - `agentbuddy skill find <query>`
  - `agentbuddy skill add <source> --all`
- `https://skills.bytedance.net/` is a portal page and preconnects
  `https://artifact-api.byted.org`; it must not be treated as a raw registry.
- `https://artifact-api.byted.org/` returns `401 Unauthorized` without auth.

## M4 Boundaries

- Pin AgentBuddy compatibility to `0.4.0`.
- Treat `skills.bytedance.net` as the AgentBuddy portal URL.
- Treat `artifact-api.byted.org` as the AgentBuddy API host, with endpoint paths
  configurable in code until a stable public schema is verified.
- M4 tests must use mocked HTTP and mocked CLI runners. Real internal network is
  allowed for smoke checks, but it is not the only acceptance path.
- Download/install fallback is out of M4 scope. M4 only reports installability
  and prepares command plans.

## Error Mapping

- Missing CLI or version mismatch: `auth error` only if auth is checked; otherwise
  `network degraded` with a clear CLI reason.
- API `401`/`403`: `auth error`.
- API timeout or transport failure: `network degraded`.
- Invalid JSON or missing required fields: `schema error`.
- Unsupported source kind: `schema error` for the source, while local sources
  continue to render.

## Marketplace Fields

M4 accepts these normalized fields from mocked AgentBuddy API responses:

- `id`
- `name`
- `description`
- `version`
- `scope`
- `star_count`
- `agents`
- `tags`
- `installable`
- `source`

Unknown fields are ignored. Missing optional fields fall back to readable
defaults; missing `id` or `name` is a schema error.
