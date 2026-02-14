# Serverless Sweep Operator Runbook (Minimal)

See `AGENTS.md` for repo constraints and expected command workflows.

This runbook covers deploy, invoke, and query steps for the AWS serverless sweep demo.

## 1) Deploy

```bash
./infra/aws_serverless_sweep/deploy_local.sh \
  -var "results_bucket_name=<unique-bucket-name>"
```

This command builds Rust parent/child Lambda binaries, packages `bootstrap` zip artifacts, and applies Terraform with stable `parent_lambda_zip` / `child_lambda_zip` inputs.

Capture outputs:

- `api_url`
- `results_bucket`
- `athena_query_policy_arn`

## 2) Invoke a Run

```bash
curl -X POST "<api_url>" \
  -H "content-type: application/json" \
  -d '{
    "run_id":"demo-run-001",
    "dimensions":{
      "commission_rate":[0.1,0.2],
      "num_drivers":[100,200],
      "num_riders":[500,700]
    },
    "shard_count":4,
    "seed":42,
    "failure_injection_shards":[1]
  }'
```

Expected parent response:

- HTTP `202`
- `shards_dispatched > 0`
- one dispatch record per shard

Ingress validation check:

```bash
curl -X POST "<api_url>" -H "content-type: application/json" -d '{"run_id":"bad"}'
```

Expected: API Gateway validation error and no parent invocation.

## 3) Verify Stored Outcomes

List partitioned results in S3:

```bash
aws s3 ls "s3://<results_bucket>/serverless-sweeps/outcomes/" --recursive
```

Check that each shard exports only Parquet objects across:

- `dataset=shard_metrics`
- `dataset=trip_data`
- `dataset=snapshot_counts`
- `dataset=shard_outcomes`

For local verification of parent/child contract behavior, run:

```bash
cargo test -p sim_serverless_sweep_lambda
```

## 4) Athena Setup + Queries

Run SQL from:

- `infra/aws_serverless_sweep/athena/create_table.sql`
- `infra/aws_serverless_sweep/athena/create_table_shard_metrics.sql`
- `infra/aws_serverless_sweep/athena/create_table_trip_data.sql`
- `infra/aws_serverless_sweep/athena/create_table_snapshot_counts.sql`
- `infra/aws_serverless_sweep/athena/repair_table.sql`
- `infra/aws_serverless_sweep/athena/repair_table_shard_metrics.sql`
- `infra/aws_serverless_sweep/athena/repair_table_trip_data.sql`
- `infra/aws_serverless_sweep/athena/repair_table_snapshot_counts.sql`
- `infra/aws_serverless_sweep/athena/query_run_level_profile.sql`
- `infra/aws_serverless_sweep/athena/query_shard_coverage.sql`
- `infra/aws_serverless_sweep/athena/query_failure_diagnostics.sql`
- `infra/aws_serverless_sweep/athena/query_trip_snapshot_join.sql`

Minimum checks:

1. `MSCK REPAIR TABLE` discovers run partitions for all four tables.
2. Run-level query returns expected shard attempts.
3. Failure-rate query returns non-zero failures when `failure_injection_shards` is used.
4. Trip/snapshot join query returns rows keyed by `(run_id, shard_id, point_index)`.
