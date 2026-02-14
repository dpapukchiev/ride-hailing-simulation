## Context

The repository currently supports local and multi-process parameter sweeps but has no cloud deployment pattern that demonstrates elastic fan-out. This change adds a minimal AWS serverless architecture with an API Gateway entrypoint that invokes a parent Lambda to partition sweep inputs and dispatch child Lambda workers for shard execution. Because this is a public repository, the design must avoid secrets in source control, enforce least-privilege IAM, and keep deployment reproducible with account-specific configuration injected at deploy/runtime.

Constraints:
- Preserve existing local experiment workflows; cloud execution is an additive path.
- Keep the architecture intentionally small for portfolio/demo value.
- Ensure deterministic execution and repeatable result aggregation across worker retries.

## Goals / Non-Goals

**Goals:**
- Demonstrate horizontally scalable sweep execution using parent/child Lambda orchestration.
- Provide a minimal HTTP-triggered run entrypoint via API Gateway for invoking the parent Lambda.
- Define a stable message contract for sweep requests, shard tasks, and shard results.
- Persist shard outputs as partitioned Parquet in S3 and support Athena-based run analysis.
- Use secure-by-default cloud patterns suitable for a public repo.
- Keep operational overhead low by using managed AWS services.

**Non-Goals:**
- Building a general-purpose workflow engine for arbitrary distributed jobs.
- Optimizing for maximum throughput or minimum cost in all workloads.
- Replacing local execution paths in `sim_experiments`.
- Introducing persistent credentials, manual secret handling, or bespoke control-plane services.

## Decisions

1. API Gateway ingress for parent invocation
   - Decision: Expose a minimal API Gateway endpoint that forwards validated sweep-run requests to the parent Lambda.
   - Rationale: Provides a clear, callable ingress path for operators and external tooling without adding custom servers.
   - Alternatives considered:
      - Direct Lambda invoke only: simpler IAM path but less discoverable and less convenient for HTTP-based automation.

2. Parent-child Lambda fan-out with asynchronous invocation
   - Decision: Use a single parent Lambda to validate request, enumerate parameter grid shards, and asynchronously invoke child Lambda for each shard without waiting for completion.
   - Rationale: Fire-and-forget orchestration avoids paying for idle parent CPU time while workers run.
   - Alternatives considered:
      - Step Functions Map: better observability/retries but adds workflow complexity for a minimal demo.
      - SQS-only workers: simpler decoupling but requires additional polling/control logic for completion tracking.

3. Bounded shard contract with deterministic partitioning
   - Decision: Parent computes deterministic shard boundaries (fixed chunk size or target shard count) and includes run_id + shard_id in payload.
   - Rationale: Enables idempotent processing and consistent aggregation under retries.
   - Alternatives considered:
     - Dynamic work stealing: improves balancing but increases coordination complexity.

4. Result collection via durable object storage + manifest
   - Decision: Each child always writes a shard outcome record to S3 under a run-scoped, partitioned path, including explicit status (`success` or `failure`) and error metadata when applicable.
   - Rationale: Mandatory writes guarantee observability of every shard attempt and produce a complete audit trail even when execution fails.
   - Alternatives considered:
     - DynamoDB-only result payloads: easier key lookups but less suitable as analytics substrate for run history.
     - Direct callback to parent: fragile because parent execution window is bounded and defeats fire-and-forget.

5. Output format and query layer: Parquet on S3 + Athena
   - Decision: Store per-shard results as Parquet files in partitioned S3 folders (for example by run_id and shard status), and perform aggregation/analysis through Athena queries over those partitions.
   - Rationale: Columnar Parquet reduces scan cost, partitioning enables selective reads, and Athena provides serverless analytics without additional compute services.
   - Alternatives considered:
     - JSON/CSV files queried ad hoc: simpler to emit but higher Athena scan cost and weaker schema discipline.
     - Custom in-Lambda aggregation service: adds compute/runtime cost and operational complexity.

6. Aggregation at Athena layer
   - Decision: Treat aggregation as query-time logic in Athena over partitioned run outputs, rather than a mandatory in-process reducer Lambda.
   - Rationale: Removes long-lived aggregation compute and aligns with analytics/reporting use cases.
   - Alternatives considered:
     - Last-writer-wins worker aggregation trigger: sensitive to duplicate completion events and late-arriving failures.
     - Dedicated aggregation Lambda: workable, but extra runtime and maintenance for functionality Athena already provides.

7. Public-repo security baseline
   - Decision: No secrets committed; all sensitive values come from environment/parameter store at deploy/runtime. IAM policies are scoped to required resources only.
   - Rationale: Aligns with public-repo safety and demonstrable cloud security practice.
   - Alternatives considered:
     - Hardcoded sample credentials or broad wildcard IAM: rejected due to security risk.

## Risks / Trade-offs

- [Cold starts increase tail latency] -> Use provisioned concurrency only if needed; keep package size small.
- [Shard skew causes slow completion] -> Use bounded shard sizing and configurable shard count.
- [Duplicate child execution from retries] -> Use idempotent object keys/versioning strategy and deterministic run_id + shard_id identity.
- [Athena partition drift or schema mismatch] -> Define explicit partition conventions and table schema; add validation checks in write path.
- [Failed shards hidden from analytics] -> Require child writes for both success and failure, including error_code and error_message fields.
- [Cost growth from over-fan-out] -> Cap max shard count per run and document expected limits.

## Migration Plan

1. Add infrastructure definitions and deploy minimal AWS resources in a sandbox account, including API Gateway routing to parent Lambda.
2. Add parent and child Lambda handlers with shared payload schema and validation.
3. Add child write path that always persists Parquet outcomes to partitioned S3 run folders.
4. Add Athena table/partition strategy and baseline queries for run-level aggregation.
5. Validate with mixed success/failure shards and confirm Athena reflects complete outcomes.
6. Rollback by disabling invocation entrypoint and leaving local sweep path unchanged.

## Open Questions

- What default partition scheme should be mandatory (`run_id`, `status`, optional date) for predictable Athena performance?
- What default shard-size heuristic best balances Lambda duration versus invocation count?
- Do we require per-run encryption key customization, or is bucket-level SSE sufficient for this demo?
