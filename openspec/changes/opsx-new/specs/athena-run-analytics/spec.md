## MODIFIED Requirements

### Requirement: Athena can query run analytics from S3 partitions
The system SHALL provide Athena-readable table definitions over the partitioned S3 layout so operators can aggregate and analyze run outcomes without copying data.

#### Scenario: Operator executes aggregate query after readiness gate
- **WHEN** an operator runs post-simulation ingestion/readiness checks for a specific `run_id`
- **THEN** Athena aggregate queries for that `run_id` return non-empty results from the relevant partitions
