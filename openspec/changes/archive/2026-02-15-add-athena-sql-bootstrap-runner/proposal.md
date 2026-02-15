## Why

Athena SQL setup in `infra/aws_serverless_sweep/athena/` is currently operator-driven and manual. This increases the chance of skipped files, wrong execution order, or drift between environments. We need a single command that can consistently bring the Athena data layer to a ready state.

## What Changes

- Add a local orchestration script that executes Athena SQL files in a deterministic order via `aws athena start-query-execution`.
- Add an ordered pipeline manifest so operators can reorder/add SQL files without changing script logic.
- Add placeholder substitution for deployment-specific values (e.g., results bucket/prefix, database name).
- Add polling/error handling so command success means Athena setup is actually complete.
- Update runbook/docs to point to the one-command bootstrap flow.

## Capabilities

### Modified Capabilities
- `athena-run-analytics`: add repeatable Athena bootstrap orchestration for setup + partition repair SQL.

## Impact

- Affected area: `infra/aws_serverless_sweep/athena/` and operator docs.
- No runtime Lambda behavior changes.
- Reduces operator setup mistakes and makes Athena readiness reproducible in CI or local ops.
