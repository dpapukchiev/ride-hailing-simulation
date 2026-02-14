# sim_serverless_sweep_lambda

AWS-facing handlers and adapters for serverless sweep orchestration.

## Ownership

- Parent and child handler flows
- Adapter traits for child invocation and object storage
- Serialization glue between API/Lambda payloads and core domain types

## Out of scope

- Deterministic shard math and contract normalization (owned by `sim_serverless_sweep_core`)
- Terraform resources and IAM policy wiring
