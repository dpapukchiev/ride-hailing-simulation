# AWS Serverless Parameter Sweep (Minimal)

See `AGENTS.md` for repo workflow expectations before changing these files.

This directory contains a minimal sandbox deployment for distributed parameter sweeps:

- API Gateway ingress (`POST /sweep-run`)
- Parent Lambda for request validation + deterministic sharding + async dispatch
- Child Lambda for shard execution + outcome persistence
- S3 partitioned outcomes for Athena analytics
- Least-privilege IAM policies for parent/child roles

## Runtime Ownership

Runtime behavior is owned by Rust crates under `crates/`:

- `crates/sim_serverless_sweep_core`: contract types, request validation, deterministic sharding, and storage key conventions
- `crates/sim_serverless_sweep_lambda`: parent/child handler flows plus AWS adapter boundaries

Terraform in this directory wires infrastructure only (API Gateway, Lambda resources, IAM, env vars, and artifact paths).
Lambda resources use the Rust custom runtime (`provided.al2023`) and packaged `bootstrap` binaries.

## Required Deploy-Time Inputs

Set these as Terraform variables (`*.tfvars`, environment, or CI secrets):

- `aws_region`: target region
- `project_name`: resource name prefix
- `results_bucket_name`: S3 bucket name for outcomes
- `results_prefix`: S3 key prefix for partitioned output
- `parent_lambda_zip`: packaged parent lambda zip path
- `child_lambda_zip`: packaged child lambda zip path
- `max_shards`: upper bound on fan-out per run
- `athena_database`: Glue/Athena database
- `athena_table`: table name for outcome records

No credentials or long-lived secrets are tracked in the repository.

## Lambda Environment Variables

Parent Lambda:

- `CHILD_LAMBDA_ARN`: worker function ARN to invoke asynchronously
- `MAX_SHARDS`: fan-out limit

Child Lambda:

- `SWEEP_RESULTS_BUCKET`: destination S3 bucket
- `SWEEP_RESULTS_PREFIX`: destination S3 partition prefix
- `MAX_SHARDS`: safety limit

## Build and Deploy

Run a single local command to build Rust Lambda binaries, package zip artifacts, and deploy Terraform:

```bash
./infra/aws_serverless_sweep/deploy_local.sh \
  -var "results_bucket_name=<bucket>"
```

The script preflights required tooling (`docker`, `aws`, `terraform`) and validates an active temporary AWS session via `aws sts get-caller-identity` before building. Rust compilation and packaging run inside the configured Docker Rust toolchain image (`SWEEP_DOCKER_IMAGE`). If your session is missing or expired, re-run AWS login first:

```bash
aws sso login
```

The script keeps the Terraform deploy interface stable by wiring:

- `parent_lambda_zip=infra/aws_serverless_sweep/dist/parent.zip`
- `child_lambda_zip=infra/aws_serverless_sweep/dist/child.zip`

Optional overrides:

- `SWEEP_LAMBDA_TARGET` (default `x86_64-unknown-linux-gnu`)
- `SWEEP_LAMBDA_PROFILE` (default `release`)
- `SWEEP_DOCKER_IMAGE` (default `docker.io/library/rust:1-bullseye`)

After deploy, copy the output `api_url` and invoke with a sweep request payload.

## Request Contract

```json
{
  "run_id": "demo-run-001",
  "dimensions": {
    "commission_rate": [0.1, 0.2],
    "num_drivers": [100, 200],
    "num_riders": [500, 800]
  },
  "shard_count": 4,
  "seed": 42,
  "failure_injection_shards": [1]
}
```

Use either `shard_count` or `shard_size`.

## Outcome Layout

Outcomes are written as Parquet-only datasets under partitioned keys:

- `<results_prefix>/dataset=shard_metrics/run_date=<yyyy-mm-dd>/run_id=<run_id>/status=success/shard_id=<id>/point_index=<point>/part-0.parquet`
- `<results_prefix>/dataset=trip_data/run_date=<yyyy-mm-dd>/run_id=<run_id>/status=success/shard_id=<id>/point_index=<point>/part-0.parquet`
- `<results_prefix>/dataset=snapshot_counts/run_date=<yyyy-mm-dd>/run_id=<run_id>/status=success/shard_id=<id>/point_index=<point>/part-0.parquet`
- `<results_prefix>/dataset=shard_outcomes/run_date=<yyyy-mm-dd>/run_id=<run_id>/status=<success|failure>/shard_id=<id>/part-0.parquet`

All datasets are joinable by `run_id`, `shard_id`, and `point_index` (where applicable).

## Athena Analytics

### Automated post-run ingestion and readiness checks

Use `xtask` to run the full post-simulation pipeline for a newly completed run. This command:

1. Ensures database + external tables exist.
2. Loads partitions for all datasets.
3. Validates run-scoped data presence and shard coverage.
4. Emits a summary and exits non-zero if readiness checks fail.

```bash
cargo run -p xtask -- post-run-ingest \
  --run-id <run_id> \
  --athena-db ride_sim_analytics \
  --athena-workgroup primary \
  --athena-query-output s3://<bucket>/<athena-query-results-prefix> \
  --results-bucket <bucket> \
  --results-prefix serverless-sweeps/outcomes \
  --expected-shards <count>
```

Required inputs:

- `--run-id`: target run to validate.
- `--athena-query-output`: S3 URI for Athena query result files.
- `--results-bucket`: bucket backing the external-table `LOCATION` values.

Optional inputs:

- `--athena-db` and `--athena-workgroup` default to `ride_sim_analytics` and `primary`.
- `--results-prefix` defaults to `serverless-sweeps/outcomes`.
- `--expected-shards` enables strict shard coverage validation and missing-shard reporting.

Environment fallbacks:

- `ATHENA_QUERY_OUTPUT` for `--athena-query-output`
- `SWEEP_RESULTS_BUCKET` for `--results-bucket`

Common failure modes and recovery:

- **Missing partitions / zero rows after fresh run**: wait for worker writes to finish, then rerun `post-run-ingest`.
- **Coverage mismatch** (`observed != expected`): inspect `query_shard_coverage.sql` and `query_failure_diagnostics.sql` for missing/failed shards.
- **Athena query failure** (permissions or location): verify workgroup access and query output S3 permissions for your operator role.

Use SQL in `infra/aws_serverless_sweep/athena/`:

- `create_table.sql`: creates outcomes/metrics/trip/snapshot external tables
- `create_table_shard_metrics.sql`, `create_table_trip_data.sql`, `create_table_snapshot_counts.sql`
- `repair_table.sql`: discovers partitions for outcomes table
- `repair_table_shard_metrics.sql`, `repair_table_trip_data.sql`, `repair_table_snapshot_counts.sql`
- `query_run_level_profile.sql`, `query_failure_diagnostics.sql`, `query_shard_coverage.sql`
- `query_trip_snapshot_join.sql`: joins per-point metrics with trip and snapshot datasets
