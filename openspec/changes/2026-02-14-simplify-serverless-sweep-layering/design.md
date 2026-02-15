## Context

See AGENTS.md for repo constraints and command expectations.

Today the sweep lifecycle is:
1) API Gateway validates request
2) Parent Lambda normalizes + shards + invokes child Lambda asynchronously
3) Child Lambda executes shard and writes Parquet outcomes

This introduces multiple contracts and ownership boundaries for one workflow. We can preserve behavior while reducing complexity by replacing cross-Lambda invocation with queue-backed dispatch and unifying runtime ownership.

## Goals / Non-Goals

**Goals**
- Preserve existing user-visible behavior (HTTP submit, distributed shard execution, Parquet outputs, Athena analytics).
- Remove parent->child Lambda invoke layer and dual artifact packaging.
- Reduce code navigation burden by minimizing crate and adapter indirection.
- Keep deterministic shard planning, idempotent shard identity, and failure visibility.

**Non-Goals**
- Rebuilding analytics schema or replacing Athena.
- Changing simulation semantics in `sim_core` / `sim_experiments`.
- Introducing Step Functions or additional orchestration services.

## Proposed Architecture

### 1) Single Lambda, dual event handlers
- One Rust Lambda binary handles two event shapes:
  - API Gateway request: validates/normalizes sweep request, computes shard plan, enqueues shard messages to SQS, returns `202`.
  - SQS record: executes one shard and writes shard outcomes.
- Handler dispatch is done via event envelope detection in one runtime entrypoint.

### 2) Queue-based fan-out
- Parent dispatch logic becomes "enqueue shard message".
- SQS + Lambda event source mapping handles retry semantics and backpressure.
- Remove direct `lambda:InvokeFunction` permissions and related adapter code.

### 3) Simplified code ownership
- Merge `sim_serverless_sweep_core` and `sim_serverless_sweep_lambda` into a unified crate (or keep crate names but enforce one public module boundary) so contracts + handlers evolve together.
- Keep pure deterministic logic (`normalize_request`, `compute_shard_plan`) in isolated modules with unit tests.
- Keep AWS-specific SDK calls in thin boundary modules.

### 4) Outcome write simplification
- Keep same S3 partition contract and Athena compatibility.
- Replace multi-step rollback-heavy success path with an idempotent write sequence using deterministic object keys and attempt-safe overwrites/versioning.
- Keep mandatory failure outcome writes for observability.

## Decisions

1. **SQS over parent->child invoke**
   - Rationale: simpler operational model (queue visibility, retries, dead-letter policy, no cross-lambda invoke plumbing).

2. **Single deployable runtime artifact**
   - Rationale: lower packaging/deploy complexity and fewer runtime drift points.

3. **Keep API Gateway entrypoint**
   - Rationale: preserves existing invocation UX and compatibility with runbook usage.

4. **Preserve Athena-facing layout**
   - Rationale: migration can be low-risk and backward-compatible for downstream analytics.

## Risks / Trade-offs

- [Mixed event handling in one binary could blur responsibilities] -> enforce explicit module boundaries (`orchestration`, `worker`, `storage`, `contracts`).
- [Queue retry duplicates] -> deterministic shard keys + idempotent outcome writes.
- [Large runs increase queue depth] -> tune batch size/concurrency and document guardrails.
- [Migration complexity] -> roll out behind a Terraform toggle and run parallel validation in sandbox.

## Migration Plan

1. Introduce SQS queue and Lambda event-source mapping in Terraform.
2. Add unified runtime entrypoint with API and SQS handler branches.
3. Replace parent->child invoke paths with enqueue paths.
4. Keep existing S3/Athena schema and verify compatibility with existing queries.
5. Remove old parent/child split resources and stale adapters after parity checks.

## Open Questions

- Should we keep one queue or split by workload class (small/large shards)?
- Do we need a DLQ for demo scope, or is on-failure destination sufficient?
- Should crate merge be done in same PR as infra shift, or staged across two changes?
