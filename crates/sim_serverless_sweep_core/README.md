# sim_serverless_sweep_core

Shared domain logic for serverless parameter sweeps.

## Ownership

- Request/response contract types and schema version constants
- Deterministic request validation and shard planning
- Partition and object-key helpers for worker output layouts

## Out of scope

- AWS Lambda runtime handlers
- AWS SDK clients (Lambda invoke, S3 writes)
- Terraform or deployment wiring
