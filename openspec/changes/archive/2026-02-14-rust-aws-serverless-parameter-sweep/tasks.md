## 1. Workspace and crate scaffolding

- [x] 1.1 Add `sim_serverless_sweep_core` and `sim_serverless_sweep_lambda` crates to the workspace with clear core-vs-adapter module boundaries
- [x] 1.2 Move Python runtime ownership assumptions out of active deploy paths and document crate ownership boundaries in crate READMEs or module docs
- [x] 1.3 Add shared schema version constants and Rust request/response types that preserve the current orchestration contract fields

## 2. Core orchestration and shard planning logic

- [x] 2.1 Implement deterministic request validation and shard planning in `sim_serverless_sweep_core` using stable shard math for `shard_count|shard_size`
- [x] 2.2 Add unit tests that prove invalid payloads fail deterministically and valid payloads generate reproducible shard plans for identical input
- [x] 2.3 Implement partition key and object key helpers for run-date/run-id/status layouts used by worker outputs

## 3. Parent and child Rust Lambda handlers

- [x] 3.1 Implement parent Lambda handler that validates payloads, creates run context, and asynchronously invokes child workers with `Event` semantics
- [x] 3.2 Implement child Lambda handler that executes assigned shard bounds via shared Rust primitives and emits structured success/failure outcomes
- [x] 3.3 Add contract-focused integration tests (or fixture replay tests) covering parent request handling and child outcome envelope compatibility

## 4. Worker execution and analytics output persistence

- [x] 4.1 Integrate `sim_experiments` shard execution path into worker handling so cloud shards use the same bounded runtime logic as local sweeps
- [x] 4.2 Reuse existing Parquet export code to produce shard metrics artifacts and upload them to S3 under Athena-queryable partition prefixes
- [x] 4.3 Persist per-shard failure records with error details and guarantee failure writes do not overwrite prior success outputs for the same shard

## 5. Deployment wiring, IAM, and packaging

- [x] 5.1 Update build/packaging workflow to produce `parent_lambda_zip` and `child_lambda_zip` from Rust Lambda binaries while keeping Terraform variable interfaces stable
- [x] 5.2 Add a local build-bundle-deploy script that runs Rust artifact build, Lambda zip packaging, and deploy in one command flow
- [x] 5.3 Add deploy-script preflight validation for a valid temporary AWS session and fail with guidance to run AWS login when missing or expired
- [x] 5.4 Ensure Terraform scope remains infrastructure wiring only (API Gateway, Lambda resources, IAM, env vars) with no embedded runtime control flow
- [x] 5.5 Tighten IAM policies for least privilege so parent and worker roles can only perform required invoke/read/write actions on configured resources

## 6. Validation, runbooks, and migration completion

- [x] 6.1 Add end-to-end verification that distributed run outputs land in expected partition paths and are queryable by Athena for success/failure aggregation
- [x] 6.2 Update operational docs/runbooks for Rust-first deployment, secret handling, rollback to prior zip artifacts, and local-vs-cloud execution expectations
- [x] 6.3 Remove Python runtime module from the active deployment path after parity checks pass and capture final migration sign-off criteria
