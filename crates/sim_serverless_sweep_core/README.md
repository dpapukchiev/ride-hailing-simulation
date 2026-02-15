# sim_serverless_sweep_core

Shared domain logic for serverless parameter sweeps.

## Ownership

- Request/response contract types and schema version constants
- Deterministic request validation and shard planning
- Partition and object-key helpers for worker output layouts

These primitives are consumed by `sim_serverless_sweep_lambda` through its runtime boundary module.

## Out of scope

- AWS Lambda runtime handlers
- AWS SDK clients (Lambda invoke, S3 writes)
- Terraform or deployment wiring
