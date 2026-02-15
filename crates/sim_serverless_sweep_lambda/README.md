# sim_serverless_sweep_lambda

AWS-facing handlers and adapters for serverless sweep orchestration.

## Ownership

- Unified runtime flow for API orchestration and SQS-driven shard execution
- Adapter traits for object storage and shard execution
- Runtime boundary module (`src/runtime.rs`) that re-exports contract/sharding/storage primitives

## Out of scope

- Terraform resources and IAM policy wiring
