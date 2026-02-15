# AWS Serverless Parameter Sweep (Minimal)

See `AGENTS.md` for repo workflow expectations before changing these files.

This directory contains a minimal sandbox deployment for distributed parameter sweeps:

- API Gateway ingress (`POST /sweep-run`)
- Unified runtime Lambda for request validation + deterministic sharding + queue dispatch + shard execution
- SQS queue for shard fan-out and retry buffering
- S3 partitioned outcomes for Athena analytics
- Least-privilege IAM policies for runtime, queue access, and analytics reads

## Runtime Ownership

Runtime behavior is owned by Rust crates under `crates/`:

- `crates/sim_serverless_sweep_core`: contract types, request validation, deterministic sharding, and storage key conventions
- `crates/sim_serverless_sweep_lambda`: unified runtime handlers and AWS adapter boundaries

Terraform in this directory wires infrastructure only (API Gateway, Lambda resources, SQS, IAM, env vars, and artifact paths).
Lambda resources use the Rust custom runtime (`provided.al2023`) and packaged `bootstrap` binaries.

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

The script preflights required tooling (`cargo`, `aws`, `terraform`) and validates an active temporary AWS session via `aws sts get-caller-identity` before building. If your session is missing or expired, re-run AWS login first:

```bash
aws sso login
```

The script wires:

- `runtime_lambda_zip=infra/aws_serverless_sweep/dist/runtime.zip`

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
