## Why

The project currently demonstrates local parameter sweeps, but it does not show a cloud-native scaling path for large sweeps. Adding a minimal AWS serverless deployment now demonstrates practical distributed execution skills while keeping the public repository safe and reproducible.

## What Changes

- Add a minimal serverless sweep architecture with an API Gateway endpoint that invokes one parent Lambda to partition parameter-space work and dispatch child Lambda invocations.
- Add child Lambda execution behavior for running assigned parameter subsets and always writing per-shard outcome records to S3, including success and failure states.
- Add Parquet-based run output layout in S3, partitioned for downstream analysis.
- Add Athena-based aggregation and analysis over run outputs, using S3 partition structure as the query boundary.
- Ensure parent invocation is fire-and-forget for child workers so orchestration does not wait idle for shard completion.
- Add secure-by-default deployment and runtime requirements for a public repo (no committed secrets, least-privilege IAM, and environment-driven configuration).
- Add operator-facing documentation for deploy, run, and observe workflows using AWS-managed services.

## Capabilities

### New Capabilities
- `serverless-sweep-orchestration`: API Gateway receives a sweep request and invokes a parent Lambda that shards parameter space and delegates shards to child workers.
- `serverless-sweep-worker-execution`: Child Lambda executes a bounded shard deterministically and writes structured per-shard outcomes to S3 for both success and failure.
- `athena-run-analytics`: Sweep output is written as partitioned Parquet in S3 and queryable in Athena for aggregation and analysis.
- `public-repo-secure-cloud-deployment`: Deployment and runtime patterns enforce no-secret-in-repo practices and least-privilege access.

### Modified Capabilities
- None.

## Impact

- Affected code areas likely include `crates/sim_experiments` (sweep execution contract), new cloud adapter/deployment assets, and documentation for cloud operation.
- Adds AWS dependencies and infrastructure definitions for API Gateway ingress, Lambda invocation, S3 Parquet output, Glue/Athena cataloging, and query execution permissions.
- Introduces cloud execution interfaces and payload schemas that must remain compatible with existing local experimentation workflows.
