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

