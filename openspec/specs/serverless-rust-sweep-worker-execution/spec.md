# serverless-rust-sweep-worker-execution Specification

## Purpose
TBD - created by archiving change rust-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
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

### Requirement: Worker exports per-point effective simulation parameters
The system SHALL persist one effective-parameter analytics record per executed point that captures the concrete simulation parameters used during worker execution.

#### Scenario: Successful shard writes effective-parameter records
- **WHEN** a worker executes a shard with one or more points
- **THEN** the worker writes per-point effective-parameter records to S3 in an Athena-queryable format
- **AND** each record includes run identifier, shard identifier, point index, and a stable parameter fingerprint

#### Scenario: Retry preserves per-point idempotency
- **WHEN** the same run/shard/point is retried due to transient failure
- **THEN** effective-parameter export remains idempotent for that run/shard/point identity
- **AND** exported records remain join-compatible with outcome records for the same point

