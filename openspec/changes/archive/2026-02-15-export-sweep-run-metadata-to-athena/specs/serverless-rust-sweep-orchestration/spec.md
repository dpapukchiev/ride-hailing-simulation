## ADDED Requirements

### Requirement: API-triggered runs export durable run-context metadata
The system SHALL persist a run-context metadata record to S3 for each API-triggered sweep run accepted by orchestration.

#### Scenario: Accepted API run writes run context record
- **WHEN** a caller submits a valid orchestration request through the API entrypoint
- **THEN** orchestration writes a run-context metadata record keyed by run identity
- **AND** the record includes run identifier, run date, shard planning summary, request provenance, and configuration fingerprint fields

#### Scenario: Rejected API run does not write run context
- **WHEN** a caller submits an invalid orchestration payload that fails validation
- **THEN** the system rejects the request
- **AND** no run-context metadata record is created for that rejected payload
