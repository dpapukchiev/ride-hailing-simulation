## ADDED Requirements

### Requirement: Athena exposes joinable run context and effective parameter datasets
The system SHALL provide Athena-readable schema definitions for run-context metadata and per-point effective simulation parameters that can be joined with run outcomes.

#### Scenario: Analyst joins outcomes to effective parameters
- **WHEN** an analyst queries run outcomes and joins on run and point identity keys
- **THEN** Athena returns outcome rows with the corresponding effective simulation parameters for each point

#### Scenario: Analyst joins outcomes to run context metadata
- **WHEN** an analyst joins run outcomes to run-context metadata by run identifier
- **THEN** Athena returns outcome aggregates enriched with run-level context fields such as request provenance and configuration fingerprint
