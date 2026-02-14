## Why

Post-simulation analytics currently rely on operators manually running Athena DDL, partition repair, and validation queries. This manual process is slow, error-prone, and easy to skip, which can leave downstream analysis unaware of missing data.

We need a single post-run pipeline that is idempotent, verifies data readiness, and fails loudly when expected results are absent.

## What Changes

- Add an operator-facing automation entrypoint (script or `xtask`) for post-simulation ingestion that runs after a sweep finishes.
- Ensure Athena/Glue database and external tables exist before querying results.
- Load the newest S3 partitions for all sweep datasets (`shard_outcomes`, `shard_metrics`, `trip_data`, `snapshot_counts`) using existing repair/partition SQL workflows.
- Add run-scoped validation queries that confirm data landed for the target `run_id` and that each expected shard has outcome rows.
- Surface a clear success/failure summary (counts by dataset, missing shard IDs, and next-step hints) to make operational triage fast.
- Document required inputs (`run_id`, region/database/table/prefix) and expected failure modes for local and CI usage.

## Capabilities

### New Capabilities
- `post-simulation-results-pipeline`: A single command orchestrates table bootstrapping, partition loading, and run-level data presence checks for newly produced simulation outputs.

### Modified Capabilities
- `athena-run-analytics`: Extend operator workflow guidance so Athena queries are gated behind an explicit readiness check proving data exists for the target run.

## Impact

- Affected areas likely include `infra/aws_serverless_sweep/athena/` (SQL orchestration/validation), `xtask/` or `scripts/` (automation entrypoint), and `infra/aws_serverless_sweep/README.md` (operator runbook updates).
- Improves reliability by making post-run ingestion idempotent and observable.
- Reduces analysis-time surprises by failing early when partitions or run data are missing.
