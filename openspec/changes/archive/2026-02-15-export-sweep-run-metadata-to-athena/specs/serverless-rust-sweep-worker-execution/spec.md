## ADDED Requirements

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
