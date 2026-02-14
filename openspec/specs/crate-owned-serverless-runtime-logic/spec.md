# crate-owned-serverless-runtime-logic Specification

## Purpose
TBD - created by archiving change rust-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
### Requirement: Runtime logic resides in Rust crates
The system SHALL implement cloud runtime and sweep execution logic under `crates/` so Terraform definitions do not embed application control flow.

#### Scenario: Runtime behavior changes without Terraform logic edits
- **WHEN** runtime orchestration behavior is updated
- **THEN** the change is implemented in Rust crate code and Terraform changes are limited to infrastructure wiring

### Requirement: Shared local and cloud execution primitives
The system SHALL expose shared execution primitives that can be used by both local sweep workflows and serverless entrypoints.

#### Scenario: Local and cloud paths use the same shard execution primitive
- **WHEN** a shard is executed locally or in serverless mode
- **THEN** both paths invoke the same Rust execution primitive with environment-specific adapters only

