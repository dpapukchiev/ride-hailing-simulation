## Context

See AGENTS.md for repo constraints and command expectations.

Simulation runs already write partitioned datasets to S3, and Athena SQL assets exist for table creation, partition repair, and analysis. The missing piece is an operationally safe, one-command post-run workflow that guarantees query readiness before analysts execute downstream reports.

## Goals / Non-Goals

**Goals**
- Provide a single idempotent command for post-simulation ingestion and validation.
- Reuse existing Athena SQL assets where possible.
- Fail early when a target `run_id` is missing, incomplete, or inconsistent across datasets.
- Produce an operator-friendly summary for CI logs and local troubleshooting.

**Non-Goals**
- Replacing simulation execution or shard orchestration.
- Redesigning storage layout, schemas, or analytics dashboards.
- Adding a long-running service for pipeline orchestration.

## Proposed Flow

1. **Bootstrap metadata objects**
   - Execute `create_database.sql` and all `create_table*.sql` files in a fixed order.
   - Treat "already exists" outcomes as success.

2. **Load/refresh partitions**
   - Execute `repair_table*.sql` statements for all datasets.
   - Optionally optimize later with run-scoped `ALTER TABLE ADD PARTITION` if needed; initial scope uses repair scripts.

3. **Run readiness validation**
   - Run validation queries that check:
     - at least one `shard_outcomes` row for `run_id`
     - presence of all expected shard IDs
     - non-zero row counts in `shard_metrics`, `trip_data`, and `snapshot_counts` for `run_id`
   - Return non-zero exit status on failed checks.

4. **Emit final report**
   - Print counts by dataset, success/failure shard counts, and missing shard IDs.
   - Include next actions (e.g., rerun repair, inspect failure diagnostics query).

## Interface

Recommended operator interface:

```bash
cargo run -p xtask -- post-run-ingest \
  --run-id <run_id> \
  --athena-db <database> \
  --athena-workgroup <workgroup> \
  --results-prefix <s3-prefix>
```

Inputs should also be overridable via environment variables to simplify CI.

## Risks / Trade-offs

- **MSCK repair latency on large buckets**: acceptable for initial version; can optimize with explicit partition adds later.
- **Athena query eventual consistency**: use bounded retries for query completion/readiness checks.
- **False negatives from delayed worker writes**: pipeline should be run after orchestration completion; diagnostics should call this out.
