## MODIFIED Requirements

### Requirement: Rust worker executes bounded sweep shards
The system SHALL execute each assigned shard in Rust from queue-delivered work messages using explicit shard bounds.

#### Scenario: Worker runs an assigned shard message
- **WHEN** a worker receives a shard message with run context and shard bounds
- **THEN** it executes only the assigned shard range using the Rust sweep runtime

### Requirement: Structured per-shard result persistence
The system SHALL persist a per-shard outcome record to S3 that includes run ID, shard ID, execution status, and output location metadata.

#### Scenario: Successful shard writes success output
- **WHEN** a shard completes without runtime errors
- **THEN** the worker writes a success outcome record and references shard output artifacts in S3

#### Scenario: Failed shard writes failure output
- **WHEN** a shard fails due to runtime or domain errors
- **THEN** the worker writes a failure outcome record with error details
- **AND** retry processing remains idempotent for the same run/shard identity
