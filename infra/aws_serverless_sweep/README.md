# AWS Serverless Parameter Sweep (Minimal)

See `AGENTS.md` for repo workflow expectations before changing these files.

This directory contains a minimal sandbox deployment for distributed parameter sweeps:

- API Gateway ingress (`POST /sweep-run`)
- Unified runtime Lambda for request validation + deterministic sharding + queue dispatch + shard execution
- SQS queue for shard fan-out and retry buffering
- S3 partitioned outcomes for Athena analytics
- Least-privilege IAM policies for runtime, queue access, and analytics reads
- S3 bucket guardrails that block all public access and deny requests from principals outside the current AWS account

## Runtime Ownership

Runtime behavior is owned by Rust crates under `crates/`:

- `crates/sim_serverless_sweep_core`: contract types, request validation, deterministic sharding, and storage key conventions
- `crates/sim_serverless_sweep_lambda`: unified runtime handlers and AWS adapter boundaries

Terraform in this directory wires infrastructure only (API Gateway, Lambda resources, SQS, IAM, env vars, and artifact paths).
Lambda resources use the Rust custom runtime (`provided.al2023`) and packaged `bootstrap` binaries.

## Runtime Observability

The runtime writes structured JSON logs to CloudWatch via stderr for each major lifecycle step:

- `sweep_runtime`: request classification (`api_gateway` vs `sqs_batch`), payload decode counts, and handler duration
- `parent_handler`: run acceptance metadata (`run_id`, `total_points`, `shard_count`) and dispatch throughput
- `child_handler`: shard start/completion/failure including per-shard `points_processed`, `duration_ms`, and `points_per_second`

Example CloudWatch Logs Insights query to trace one run across parent and child phases:

```sql
fields @timestamp, @message
| filter @message like /"run_id":"demo-run-001"/
| sort @timestamp asc
| limit 200
```

Example CloudWatch Logs Insights query for shard throughput during a run:

```sql
fields @timestamp, @message
| filter @message like /"component":"child_handler"/
| filter @message like /"event":"shard_completed"/
| parse @message '"points_processed":*,' as points_processed
| parse @message '"points_per_second":*,' as points_per_second
| stats avg(to_double(points_per_second)) as avg_points_per_second,
        sum(to_bigint(points_processed)) as total_points
```

## Required Deploy-Time Inputs

Set these as Terraform variables (`*.tfvars`, environment, or CI secrets):

- `aws_region`: target region
- `project_name`: resource name prefix
- `results_bucket_name`: S3 bucket name for outcomes
- `results_prefix`: S3 key prefix for partitioned output
- `runtime_lambda_zip`: packaged runtime lambda zip path
- `max_shards`: upper bound on fan-out per run
- `athena_database`: Glue/Athena database
- `athena_table`: table name for outcome records

No credentials or long-lived secrets are tracked in the repository.

## Lambda Environment Variables

Unified Runtime Lambda:

- `SHARD_QUEUE_URL`: queue URL for shard work dispatch
- `SWEEP_RESULTS_BUCKET`: destination S3 bucket
- `SWEEP_RESULTS_PREFIX`: destination S3 partition prefix
- `MAX_SHARDS`: safety fan-out limit

## Build and Deploy

Run a single local command to build the Rust Lambda binary, package zip artifact, and deploy Terraform:

```bash
./infra/aws_serverless_sweep/deploy_local.sh \
  -var "results_bucket_name=<bucket>"
```

The script preflights required tooling (`docker`, `aws`, `terraform`) and validates an active temporary AWS session via `aws sts get-caller-identity` before building. Rust compilation and packaging run inside a reusable Docker builder image derived from the configured Rust toolchain image (`SWEEP_DOCKER_IMAGE`).

On the first run, the script builds runtime artifacts and records metadata in `infra/aws_serverless_sweep/dist/runtime-build.metadata`. On subsequent runs, if target/profile/toolchain/source fingerprint are unchanged, it reuses `dist/runtime.zip` and skips rebuild work.

If your session is missing or expired, re-run AWS login first:

```bash
aws sso login
```

The script wires:

- `runtime_lambda_zip=infra/aws_serverless_sweep/dist/runtime.zip`

Optional overrides:

- `SWEEP_LAMBDA_TARGET` (default `x86_64-unknown-linux-gnu`)
- `SWEEP_LAMBDA_PROFILE` (default `release`)
- `SWEEP_DOCKER_IMAGE` (default `docker.io/library/rust:1-bullseye`)
- `SWEEP_BUILDER_IMAGE` (default derived local tag, e.g. `ride-sweep-builder:<image-slug>`)
- `SWEEP_DOCKER_PULL_POLICY` (`if-missing` default; also supports `always` and `never`)
- `SWEEP_FORCE_REBUILD=1` to ignore cache and rebuild artifacts
- `--force-rebuild` CLI flag (same behavior as `SWEEP_FORCE_REBUILD=1`)

Use `--force-rebuild` after toolchain updates or when diagnosing stale local build state.

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
- `<results_prefix>/dataset=shard_outcomes/run_date=<yyyy-mm-dd>/run_id_partition=<run_id>/status_partition=<success|failure>/shard_id_partition=<id>/part-0.parquet`
- `<results_prefix>/dataset=run_context/run_date=<yyyy-mm-dd>/run_id_partition=<run_id>/status_partition=accepted/part-0.parquet`
- `<results_prefix>/dataset=effective_parameters/run_date=<yyyy-mm-dd>/run_id_partition=<run_id>/status_partition=success/shard_id_partition=<id>/point_index_partition=<point>/part-0.parquet`

`run_date` is set once when the parent request dispatches shard messages and is carried in each shard payload. SQS retries or DLQ redrives for the same `run_id`/`shard_id` therefore overwrite the same partition path instead of creating a new date partition.

All datasets are joinable by `run_id`, `shard_id`, and `point_index` (where applicable).

Run-context records are emitted once when orchestration accepts a request, so they remain available even if one or more shard executions later fail. Effective-parameter records are emitted per successful point write and use deterministic object keys, so retries overwrite the same S3 object for the same run/shard/point identity.

Each exported analytics record includes a `record_schema` version field (`v1` today). Downstream queries should filter or branch on `record_schema` when reading across multiple deployment versions.

## Athena Analytics

Bootstrap the Athena data layer in one command:

```bash
python3 infra/aws_serverless_sweep/athena/apply_athena_sql.py \
  --query-results-s3 "s3://<athena-query-results-bucket>/queries/" \
  --results-bucket "<results-bucket>" \
  --results-prefix "serverless-sweeps/outcomes" \
  --workgroup "primary"
```

To only discover newly written partitions without recreating or touching database/table DDL, run:

```bash
python3 infra/aws_serverless_sweep/athena/apply_athena_sql.py \
  --execution-mode "partitions-only" \
  --query-results-s3 "s3://<athena-query-results-bucket>/queries/" \
  --workgroup "primary"
```

`partitions-only` mode launches partition repair statements concurrently (default `--max-concurrent 4`; tune based on Athena workgroup limits).

`--results-bucket` accepts either a plain bucket name (`my-bucket`) or an S3 URI (`s3://my-bucket[/optional/prefix]`).

To add or reorder setup queries, edit `infra/aws_serverless_sweep/athena/athena_bootstrap.plan`.
To customize partition-only refresh order, edit `infra/aws_serverless_sweep/athena/athena_partitions.plan`.

Athena SQL assets live in `infra/aws_serverless_sweep/athena/`:

- `create_table.sql`: creates outcomes/metrics/trip/snapshot external tables
- `create_table_run_context.sql`, `create_table_effective_parameters.sql`
- `create_table_shard_metrics.sql`, `create_table_trip_data.sql`, `create_table_snapshot_counts.sql`
- `repair_table.sql`: discovers partitions for outcomes table
- `repair_table_run_context.sql`, `repair_table_effective_parameters.sql`
- `repair_table_shard_metrics.sql`, `repair_table_trip_data.sql`, `repair_table_snapshot_counts.sql`
- `query_run_level_profile.sql`, `query_failure_diagnostics.sql`, `query_shard_coverage.sql`
- `query_trip_snapshot_join.sql`: joins per-point metrics with trip and snapshot datasets
- `query_outcome_configuration_smoke.sql`: validates joins across outcomes, run context, and effective parameters for one run

`query_run_level_profile.sql` now includes run-level throughput columns (`successful_points`, `run_window_seconds`, `points_per_second`) so you can estimate scaling and end-to-end runtime for future experiment sizes.
