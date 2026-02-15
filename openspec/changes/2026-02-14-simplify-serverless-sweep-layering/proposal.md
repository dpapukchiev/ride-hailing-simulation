## Why

The current serverless sweep path works, but it is harder to reason about than necessary for the outcomes it produces. The implementation spreads orchestration logic across API Gateway validation, a parent Lambda, child Lambda handlers, adapter traits, a separate core crate, and Terraform wiring. This layering improves separation, but it also increases the cognitive load for common changes (payload schema updates, failure handling, and run-level observability).

The goal is to keep the same functional result (distributed shard execution with Parquet outcomes queryable in Athena) while reducing moving parts and handoff boundaries.

## Current Complexity Review

- Two runtime Lambdas (parent + child) require cross-function contracts, async invoke error paths, and dual packaging/deploy artifacts.
- Runtime logic is split across `sim_serverless_sweep_core` and `sim_serverless_sweep_lambda`; contributors must navigate multiple crates for one request lifecycle.
- Parent/child adapter abstraction adds indirection even when only one concrete AWS implementation is used in production.
- Infra currently manages API Gateway model validation plus Lambda integration plus invoke IAM for parent->child fan-out.
- Outcome persistence supports multiple datasets with repeated path/key assembly and rollback behavior.

## What Changes

- Replace parent/child Lambda fan-out with a **single runtime Lambda + SQS queue** pattern:
  - API request path: validate + shard + enqueue work items.
  - Worker path: consume queued shard items and execute shard.
- Consolidate runtime code into one crate/module boundary (domain + aws-runtime modules in the same crate), keeping pure deterministic logic testable without duplicating payload glue across crates.
- Keep deterministic shard planning and run_id/shard_id identity guarantees unchanged.
- Preserve durable Parquet outcomes and Athena queryability, but simplify write semantics to a single idempotent writer contract per shard attempt.
- Reduce deployment surface from two Lambda artifacts to one Lambda artifact plus one queue/event-source mapping.

## Capabilities

### Modified Capabilities
- `serverless-rust-sweep-orchestration`: move from parent->child Lambda invocation to enqueue-based shard dispatch.
- `serverless-rust-sweep-worker-execution`: worker execution is triggered by SQS records instead of direct Lambda invoke payloads.
- `crate-owned-serverless-runtime-logic`: collapse multi-crate runtime layering into a simpler single-crate ownership model.

### New Capabilities
- None (this is a simplification/refactor with equivalent business outcomes).

## Impact

- Affected code areas: `crates/sim_serverless_sweep_lambda`, `crates/sim_serverless_sweep_core` (or merged replacement), and `infra/aws_serverless_sweep/terraform`.
- Operationally simpler deploy/runbook: one Lambda binary to package, one queue to inspect, fewer cross-service permissions.
- Improved maintainability for schema, error handling, and retry logic by removing parent->child invocation plumbing.
