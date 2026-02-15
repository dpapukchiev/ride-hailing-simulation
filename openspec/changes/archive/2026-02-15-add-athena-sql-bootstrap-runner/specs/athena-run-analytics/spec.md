## MODIFIED Requirements

### Requirement: Athena can query run analytics from S3 partitions
The system SHALL provide Athena-readable table definitions over the partitioned S3 layout so operators can aggregate and analyze run outcomes without copying data.

#### Scenario: Operator executes aggregate query
- **WHEN** an operator runs an Athena query for a specific sweep run
- **THEN** Athena returns aggregated metrics using only the relevant S3 partitions

### Requirement: Athena bootstrap SQL can be applied as a single ordered workflow
The system SHALL provide a one-command workflow that executes the required Athena setup SQL files in deterministic order and fails immediately on statement errors.

#### Scenario: Operator prepares Athena data layer
- **GIVEN** the operator provides AWS region, workgroup, Athena query output location, and S3 results bucket/prefix
- **WHEN** the operator runs the Athena bootstrap command
- **THEN** create-database/create-table/repair statements are executed in configured order
- **AND** the command exits successfully only after all statements complete in `SUCCEEDED` state
- **AND** the command emits the failing query execution id and reason when any statement fails

#### Scenario: Operator customizes workflow order
- **WHEN** the operator edits the SQL manifest to add/reorder files
- **THEN** subsequent bootstrap runs follow the new manifest order without script code changes
