# Serverless Sweep Operator Runbook (Minimal)

See `AGENTS.md` for repo constraints and expected command workflows.

This runbook covers deploy, invoke, rollback, and verification steps for the AWS serverless sweep flow.

## 1) Deploy

```bash
./infra/aws_serverless_sweep/deploy_local.sh \
  -var "results_bucket_name=<unique-bucket-name>"
```

This command builds Rust parent/child Lambda binaries, packages `bootstrap` zip artifacts, and applies Terraform with stable `parent_lambda_zip` / `child_lambda_zip` inputs.

Secret and credential posture:

- Never commit cloud credentials or secret files.
- Use short-lived credentials only (`aws sso login` or equivalent temporary session).
- `deploy_local.sh` preflights credentials via `aws sts get-caller-identity` and fails fast if the session is expired.

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

For IAM least-privilege scope checks, run Terraform validation and then inspect role policies in plan output:

```bash
terraform -chdir=infra/aws_serverless_sweep/terraform validate
terraform -chdir=infra/aws_serverless_sweep/terraform plan
```

## 4) Athena Setup + Queries

Run one command to apply Athena setup SQL in deterministic order:

```bash
python3 infra/aws_serverless_sweep/athena/apply_athena_sql.py \
  --query-results-s3 "s3://<athena-query-results-bucket>/queries/" \
  --results-bucket "<results-bucket>" \
  --results-prefix "serverless-sweeps/outcomes" \
  --workgroup "primary"
```

`--results-bucket` accepts either a plain bucket name (`my-bucket`) or an S3 URI (`s3://my-bucket[/optional/prefix]`).

The order is controlled by `infra/aws_serverless_sweep/athena/athena_bootstrap.plan`; edit this file to add or reorder SQL steps.

Additional Athena SQL files are available in:

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

Run `query_run_level_profile.sql` and `query_failure_diagnostics.sql` with your target `run_id` to verify outcomes are queryable end to end.

## 5) Rollback

If a new Rust runtime deployment regresses:

1. Re-package a known-good artifact pair (`parent.zip`, `child.zip`) or retrieve prior build outputs.
2. Re-apply Terraform with the known-good zip paths:

```bash
terraform -chdir=infra/aws_serverless_sweep/terraform apply \
  -var "parent_lambda_zip=<known-good-parent.zip>" \
  -var "child_lambda_zip=<known-good-child.zip>"
```

3. Re-run the invocation and Athena checks above.

## 6) Local vs Cloud Expectations

- Local Rust tests validate contracts, deterministic sharding, and handler behavior.
- Cloud validation confirms deployment wiring, S3 persistence, and Athena queryability against real IAM and API Gateway.
- The active deployment path is Rust-only (legacy Python runtime module removed).

## 7) Migration Sign-off Criteria

The migration is considered complete when all are true:

1. Rust parent/child handlers pass local crate tests.
2. Deploy script succeeds with temporary credentials only.
3. Results land in partitioned `dataset=shard_*` S3 paths for the run.
4. Athena queries return success/failure aggregation for the run.
5. Terraform remains infrastructure wiring only; runtime behavior changes are done in `crates/`.
