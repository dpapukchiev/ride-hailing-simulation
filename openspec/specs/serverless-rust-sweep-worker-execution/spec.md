# serverless-rust-sweep-worker-execution Specification

## Purpose
TBD - created by archiving change rust-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
### Requirement: Rust worker executes bounded sweep shards
The system SHALL execute each assigned shard in Rust using shared experiment code and explicit shard bounds.

#### Scenario: Worker runs an assigned shard
- **WHEN** a worker receives a shard payload with run context and shard bounds
- **THEN** it executes only the assigned shard range using the Rust sweep runtime

### Requirement: Structured per-shard result persistence
The system SHALL persist a per-shard outcome record to S3 that includes run ID, shard ID, execution status, and output location metadata.

#### Scenario: Successful shard writes success output
- **WHEN** a shard completes without runtime errors
- **THEN** the worker writes a success outcome record and references the shard output object in S3

#### Scenario: Failed shard writes failure output
- **WHEN** a shard fails due to runtime or domain errors
- **THEN** the worker writes a failure outcome record with error details and does not overwrite existing success outcomes for that shard

