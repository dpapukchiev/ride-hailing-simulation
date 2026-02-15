# crate-owned-serverless-runtime-logic Specification

## Purpose
TBD - created by archiving change rust-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
### Requirement: Runtime logic resides in Rust crates
The system SHALL implement cloud runtime and sweep execution logic under `crates/` with a simplified ownership model that minimizes cross-crate orchestration indirection.

#### Scenario: Runtime behavior changes without Terraform logic edits
- **WHEN** runtime orchestration behavior is updated
- **THEN** the change is implemented in Rust crate code and infrastructure changes remain limited to wiring

### Requirement: Shared local and cloud execution primitives
The system SHALL expose shared execution primitives used by local sweep workflows and queue-driven serverless worker execution.

#### Scenario: Local and cloud paths use the same shard execution primitive
- **WHEN** a shard is executed locally or in serverless mode
- **THEN** both paths invoke the same Rust execution primitive with only environment-specific adapter differences

### Requirement: Orchestration and worker contracts evolve together
The system SHALL keep orchestration and worker payload contracts in a single runtime ownership boundary to reduce schema drift risk.

#### Scenario: Payload schema update remains coherent
- **WHEN** a contract field is added or modified
- **THEN** orchestration enqueue logic and worker decode logic are changed and tested in one code ownership boundary

