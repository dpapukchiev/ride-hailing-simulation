## ADDED Requirements

### Requirement: API-driven sweep orchestration contract
The system SHALL retain the existing API Gateway and parent Lambda flow while accepting a stable, versioned orchestration payload that can be executed by Rust runtime code.

#### Scenario: Valid orchestration request starts a run
- **WHEN** a caller submits a valid orchestration payload through the existing API entrypoint
- **THEN** the parent Lambda starts a sweep run using Rust orchestration logic and returns a run identifier

#### Scenario: Invalid payload is rejected deterministically
- **WHEN** a caller submits a payload that violates the orchestration contract
- **THEN** the system rejects the request with a validation error and does not start a run

### Requirement: Deterministic shard planning
The system SHALL produce deterministic shard plans for identical run inputs so repeated invocations generate the same shard boundaries and execution metadata.

#### Scenario: Repeated requests produce identical plans
- **WHEN** the same orchestration input is processed multiple times
- **THEN** each generated shard plan contains the same shard count, shard ranges, and run configuration fingerprint
