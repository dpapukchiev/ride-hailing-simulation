## 1. Infrastructure Skeleton and Configuration

- [x] 1.1 Add minimal IaC resources for API Gateway, parent Lambda, child Lambda, S3 bucket/prefixes, and Athena query access in a sandbox account configuration.
- [x] 1.2 Define environment-driven configuration inputs (region, bucket, prefixes, role ARNs, limits) and document required deploy-time variables without embedding secrets.
- [x] 1.3 Configure API Gateway request validation so malformed sweep-run payloads are rejected before invoking the parent Lambda.

## 2. Parent Orchestration Flow

- [x] 2.1 Implement parent Lambda request schema validation for required sweep and sharding parameters.
- [x] 2.2 Implement deterministic shard partitioning that covers the full parameter space exactly once with non-overlapping shard bounds.
- [x] 2.3 Implement asynchronous fire-and-forget dispatch of one child invocation per shard and return parent completion without waiting for worker results.

## 3. Child Worker Execution and Outcome Writes

- [x] 3.1 Implement child Lambda handler to execute only the assigned shard bounds using deterministic simulation inputs derived from payload fields.
- [x] 3.2 Implement per-shard success outcome writes to S3 as Parquet-compatible records with run_id, shard_id, status, and output metadata.
- [x] 3.3 Implement per-shard failure outcome writes to S3 with error_code/error_message fields so failed attempts are always represented.

## 4. S3 Partitioning and Athena Analytics

- [x] 4.1 Define and enforce S3 partition conventions for run analytics (at minimum run_id and status, with optional date partition).
- [x] 4.2 Create Athena table definitions over partitioned Parquet outputs and verify partition discovery/queryability.
- [x] 4.3 Add baseline Athena queries for run-level aggregation metrics and failure-rate reporting.

## 5. Security and Least-Privilege Hardening

- [x] 5.1 Scope IAM permissions for parent Lambda to only required invoke, logging, and configuration reads.
- [x] 5.2 Scope IAM permissions for child Lambda to only required S3 write/read prefixes and required execution actions.
- [x] 5.3 Add repository checks/documentation to ensure no cloud credentials or plaintext secrets are tracked.

## 6. Verification and Demo Readiness

- [x] 6.1 Run an end-to-end sweep with mixed successful and failing shards and verify complete shard coverage in stored outcomes.
- [x] 6.2 Validate that invalid ingress requests are rejected and do not trigger parent dispatch.
- [x] 6.3 Capture a minimal operator runbook showing deploy, invoke, and Athena query steps for portfolio/demo usage.
