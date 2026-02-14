## Context

The repository already includes a minimal AWS serverless sweep skeleton under `infra/aws_serverless_sweep/` with API Gateway ingress, parent/child Lambda orchestration, and S3 outcome writes, but the runtime path is a placeholder Python module. In parallel, the real sweep logic and parameter tooling already live in Rust (`crates/sim_experiments`).

The current mismatch creates two problems: (1) we cannot run realistic distributed sweeps using the same Rust logic we trust locally, and (2) application behavior is too coupled to infra layout because runtime code sits in infra folders.

Constraints:
- Keep the existing infrastructure shape (API Gateway -> parent Lambda -> async child Lambdas -> S3/Athena) to avoid re-architecting a working skeleton.
- Preserve local experiment flows; cloud distribution is additive, not a replacement.
- Keep public-repo safety posture (no secrets in repo, least-privilege IAM, reproducible deployment).

## Goals / Non-Goals

**Goals:**
- Replace Python Lambda runtime logic with Rust while preserving request/result contracts.
- Move cloud runtime/orchestration code ownership into `crates/` so Terraform only wires infrastructure.
- Reuse `sim_experiments` parameter-space and deterministic execution behavior in worker shards.
- Keep async parent fan-out and per-shard durable success/failure records in S3.
- Reuse the existing Parquet exporter in `sim_experiments` and persist shard/run outputs to S3 with Athena-friendly partitioning for fast querying and exploration.

**Non-Goals:**
- Replacing Terraform with another IaC system.
- Designing a new workflow engine (for example Step Functions orchestration) in this change.
- Optimizing for maximal throughput/cost for every workload.
- Removing the existing local sweep entrypoints.

## Decisions

1. Keep existing infra topology; swap runtime implementation language
   - Decision: Preserve API Gateway + parent Lambda + async child Lambda + S3/Athena topology, but implement parent and child handlers in Rust.
   - Rationale: Fastest path to production-like distributed sweeps with minimal infrastructure churn.
   - Alternatives considered:
     - Rebuild orchestration with Step Functions now: better workflow visibility, but larger scope and slower migration.
     - Keep Python for parent and only migrate child: reduces effort but keeps split-language runtime and duplicated contract logic.

2. Place runtime code in workspace crates
   - Decision: Move Lambda business/runtime logic from `infra/aws_serverless_sweep/lambda/serverless_sweep.py` into Rust crates under `crates/`.
   - Rationale: Decouples app behavior from Terraform, enables normal Rust testing/versioning, and reuses existing experiment modules.
   - Planned boundary:
     - `crates/sim_serverless_sweep_core` (new): request validation, deterministic shard math, payload/result schemas, partition key generation.
     - `crates/sim_serverless_sweep_lambda` (new): AWS Lambda handlers (parent/child), AWS SDK invocation + S3 writes, env-var wiring.
     - `crates/sim_experiments` (existing): shard execution and metrics computation reused by worker path.
   - Alternatives considered:
     - Put handler code in `infra/`: keeps infra coupling and weakens reuse.
     - Add only one monolithic crate: simpler at first, but weaker separation between cloud adapters and domain logic.

3. Keep contract compatibility and version it explicitly
   - Decision: Retain the existing request contract fields (`run_id`, `dimensions`, `shard_count|shard_size`, `seed`, optional failure injection for tests) and outcome envelope shape, while adding explicit schema version constants in Rust.
   - Rationale: Avoids Terraform/API churn and keeps runbook/test compatibility during migration.
   - Alternatives considered:
     - Introduce a brand-new Rust-native API contract now: cleaner long term, but breaks existing artifacts and tests immediately.

4. Parent remains async dispatcher, child remains durable writer
    - Decision: Parent validates + computes deterministic shards + invokes child Lambdas with async invocation (`Event` semantics). Child always persists a success or failure record to S3.
    - Rationale: Preserves non-blocking orchestration and complete shard observability.
    - Alternatives considered:
      - Parent waits for child completion: simpler run status, but wastes Lambda runtime and reduces scale.

5. Use existing Parquet export path and upload artifacts to S3
   - Decision: Worker writes shard metrics using `sim_experiments` Parquet export code (currently in `crates/sim_experiments/src/export/parquet.rs`), then uploads the produced object to S3.
   - Rationale: Avoids duplicate serialization logic, keeps schema aligned with local experiment exports, and gives immediate Athena compatibility.
   - Output layout decision (Hive-style partitions):
     - `s3://<bucket>/<prefix>/dataset=shard_metrics/run_date=<yyyy-mm-dd>/run_id=<run_id>/status=<success|failure>/part-<shard_id>.parquet`
     - `s3://<bucket>/<prefix>/dataset=shard_errors/run_date=<yyyy-mm-dd>/run_id=<run_id>/status=failure/part-<shard_id>.parquet`
   - Queryability rationale:
     - Partition on low/medium-cardinality filters used in exploration (`run_date`, `run_id`, `status`).
     - Keep `shard_id` as a column/file suffix rather than a partition key to avoid partition explosion.
   - Alternatives considered:
     - Keep JSON-only outcomes: easier migration but weaker schema discipline and higher Athena scan cost.
     - Partition by `shard_id`: simpler point lookups but poor partition scaling for large runs.

6. Build and packaging become Rust-first
   - Decision: Terraform artifact inputs continue to be `parent_lambda_zip` and `child_lambda_zip`, but these zips now package Rust binaries/bootstraps built for Lambda runtime.
   - Rationale: Keeps Terraform interface stable while switching implementation language.
   - Alternatives considered:
     - Change Terraform module interface immediately to container images: useful later, but unnecessary for this migration step.

7. Terraform scope narrows to infrastructure wiring
    - Decision: Terraform manages IAM, API Gateway, Lambda resources, and env vars only; no embedded runtime logic assumptions beyond artifact paths and env contracts.
    - Rationale: Clear infra/app ownership boundary and easier future runtime iteration.
    - Alternatives considered:
      - Keep Terraform/docs tightly coupled to Python-specific packaging commands: blocks language migration and increases maintenance cost.

8. Provide a local build-bundle-deploy script gated by AWS temporary login
   - Decision: Add a repo script that performs Rust Lambda build, zip packaging, and deploy as one local command, and preflights AWS identity to ensure a valid temporary session exists.
   - Rationale: Keeps early operational flow simple for local iteration while preserving public-repo safety by requiring short-lived credentials from an explicit AWS login step.
   - Alternatives considered:
     - Manual multi-command deploy per run: error-prone and harder to repeat consistently.
     - CI-first deploy automation now: useful later, but out of scope while deployment is local-only.

## Risks / Trade-offs

- [Rust Lambda packaging friction on Windows/Linux CI targets] -> Standardize cross-compilation/packaging commands in workspace tooling and document reproducible build steps.
- [Behavior drift from Python implementation during migration] -> Add contract tests that replay known request/shard/outcome fixtures against Rust implementation.
- [Tighter dependency graph across new crates] -> Keep core crate free of AWS SDK types; isolate cloud adapters in lambda crate.
- [Large parameter-space materialization in worker] -> Reuse bounded shard execution patterns and validate shard bounds before evaluation.
- [Parquet schema drift across local/cloud paths] -> Make `sim_experiments` Parquet schema the single source of truth and validate with integration tests that read produced S3 objects.
- [Operational confusion during cutover] -> Keep old Python path only as temporary reference docs/tests until Rust parity is verified, then remove.
- [Failed deploy attempts due to expired local AWS sessions] -> Add deploy-script preflight check with actionable error message instructing operators to re-run AWS login.

## Migration Plan

1. Create Rust crates for shared serverless core logic and Lambda handlers; wire them into workspace.
2. Port validation, sharding, and outcome-key logic from Python to Rust with unit tests using existing fixture-like payloads.
3. Integrate worker execution with `sim_experiments` so shard runs use existing Rust parameter-sweep logic.
4. Add parent/child Lambda handlers in Rust with async invoke and S3 writes of Parquet shard outputs.
5. Update packaging flow to produce `parent_lambda_zip` and `child_lambda_zip` from Rust artifacts while keeping Terraform variable contract unchanged.
6. Update runbook/tests to invoke Rust handlers and verify S3 Parquet partition layout + Athena query compatibility.
7. Remove Python runtime module from active deploy path once parity checks pass.

Rollback strategy:
- Keep Terraform and API contracts stable so deployment can be pointed back to previous Lambda zip artifacts if Rust runtime regressions appear.

## Open Questions

- Do we want one Lambda crate with two binaries, or separate parent/child Lambda crates for clearer ownership?
- Which packaging toolchain should be canonical in this repo (`cargo lambda`, `cross`, or custom xtask) for deterministic CI builds?
