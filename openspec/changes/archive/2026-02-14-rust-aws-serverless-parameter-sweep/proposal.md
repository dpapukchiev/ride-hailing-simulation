## Why

Our parameter sweeps take hours on a single machine, which blocks iteration and limits experiment scale. We already have an AWS serverless skeleton, but its placeholder Python path and infra-coupled layout prevent us from using the existing Rust sweep engine cleanly.

## What Changes

- Reuse the existing serverless sweep skeleton (API Gateway + parent/worker Lambda flow) instead of designing a new architecture.
- Replace placeholder Python worker/orchestration code paths with Rust implementations that run sweep shards deterministically.
- Move runtime sweep/orchestration logic into `crates/` so cloud execution behavior lives in Rust crates rather than Terraform-managed infra glue.
- Persist per-shard success and failure outputs to S3 in a structured layout suitable for downstream aggregation.
- Add Athena-queryable output conventions for run-level analysis across distributed shard results.
- Keep local sweep workflows intact while adding a compatible cloud execution contract.
- Add secure deployment and runtime requirements (no committed secrets, least-privilege IAM, env-driven config).

## Capabilities

### New Capabilities
- `serverless-rust-sweep-orchestration`: Existing API Gateway and parent Lambda flow is retained, while orchestration logic is implemented in Rust and invoked through a stable payload contract.
- `serverless-rust-sweep-worker-execution`: Existing worker path is migrated from Python placeholders to Rust so bounded shards execute using shared experiment code and write structured per-shard outcomes to S3.
- `crate-owned-serverless-runtime-logic`: Cloud runtime and sweep execution code is hosted under `crates/` to decouple application logic from Terraform definitions and reduce infra/application coupling.
- `distributed-sweep-results-analytics`: Distributed outputs are queryable for aggregation and analysis using S3 partitioning and Athena-compatible formats.
- `public-repo-secure-cloud-deployment`: Deployment and runtime patterns enforce no-secret-in-repo practices and least-privilege access.

### Modified Capabilities
- None.

## Impact

- Affected code areas likely include `crates/sim_experiments` plus one or more new Rust crates for Lambda entrypoints/orchestration, with Terraform focusing on infrastructure wiring only.
- Adds AWS dependencies and infra configuration for API Gateway ingress, Lambda execution, S3 output storage, and Athena/Glue query access.
- Replaces Python-oriented packaging assumptions with Rust Lambda build/deploy packaging requirements while keeping compatibility with existing local experimentation workflows.
