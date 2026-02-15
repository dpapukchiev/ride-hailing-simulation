# athena-run-analytics Specification

## Purpose
TBD - created by archiving change minimal-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
### Requirement: Sweep outcomes are stored as partitioned parquet
The system SHALL store sweep output data in S3 using Parquet format and a partition layout that enables selective query scanning by run and shard dimensions.

#### Scenario: Output writer creates partitioned parquet objects
- **WHEN** worker outputs are finalized for a shard
- **THEN** parquet objects are written under the configured S3 prefix using the required partition keys

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

### Requirement: Athena exposes joinable run context and effective parameter datasets
The system SHALL provide Athena-readable schema definitions for run-context metadata and per-point effective simulation parameters that can be joined with run outcomes.

#### Scenario: Analyst joins outcomes to effective parameters
- **WHEN** an analyst queries run outcomes and joins on run and point identity keys
- **THEN** Athena returns outcome rows with the corresponding effective simulation parameters for each point

#### Scenario: Analyst joins outcomes to run context metadata
- **WHEN** an analyst joins run outcomes to run-context metadata by run identifier
- **THEN** Athena returns outcome aggregates enriched with run-level context fields such as request provenance and configuration fingerprint

