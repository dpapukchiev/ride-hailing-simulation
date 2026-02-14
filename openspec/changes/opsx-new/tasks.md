## 1. Pipeline Entry Point

- [x] 1.1 Add an operator command (`xtask` or script) that orchestrates post-run ingestion for a provided `run_id`.
- [x] 1.2 Support required runtime parameters (Athena database/workgroup, output location, results prefix) via args + env fallback.

## 2. Metadata Bootstrap and Partition Loading

- [x] 2.1 Execute Athena DDL for database/table creation idempotently.
- [x] 2.2 Execute partition loading/repair SQL for `shard_outcomes`, `shard_metrics`, `trip_data`, and `snapshot_counts`.

## 3. Readiness Validation

- [x] 3.1 Add run-scoped checks confirming rows exist for the target `run_id` in each required dataset.
- [x] 3.2 Validate shard coverage using expected shard count and/or shard ID presence in `shard_outcomes`.
- [x] 3.3 Exit non-zero when checks fail and print actionable diagnostics.

## 4. Operator Documentation

- [x] 4.1 Update `infra/aws_serverless_sweep/README.md` with the post-run command and required inputs.
- [x] 4.2 Document failure modes and recovery steps (missing partitions, empty run, partial shard outcomes).
