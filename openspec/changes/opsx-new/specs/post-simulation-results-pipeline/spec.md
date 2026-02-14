## ADDED Requirements

### Requirement: Single-command post-simulation ingestion
The system SHALL provide one operator command that executes the full post-simulation ingestion sequence for a specified `run_id`.

#### Scenario: Operator runs ingestion command
- **WHEN** an operator triggers post-run ingestion with required configuration inputs
- **THEN** the command executes metadata bootstrap, partition loading, and run-readiness validation as one workflow

### Requirement: Idempotent metadata bootstrap and partition loading
The system SHALL ensure Athena database/table objects exist and load partitions without failing on already-existing resources.

#### Scenario: Re-running ingestion for an existing run
- **WHEN** ingestion is executed multiple times for the same environment and `run_id`
- **THEN** database/table setup and partition loading complete without duplicate-resource errors

### Requirement: Run-readiness validation with explicit failure
The system SHALL verify required datasets contain rows for the target `run_id` and fail with diagnostics when data is missing.

#### Scenario: Missing dataset rows for run
- **WHEN** at least one required dataset has zero rows for the target `run_id`
- **THEN** the command exits with failure and reports which dataset checks failed
