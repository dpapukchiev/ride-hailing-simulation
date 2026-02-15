# sim_serverless_sweep_lambda

AWS-facing handlers and adapters for serverless sweep orchestration.

## Ownership

- Unified runtime flow for API orchestration and SQS-driven shard execution
- Adapter traits for shard dispatch and object storage
- Serialization glue between API/Lambda payloads and core domain types

## Out of scope

- Deterministic shard math and contract normalization (owned by `sim_serverless_sweep_core`)
- Terraform resources and IAM policy wiring
