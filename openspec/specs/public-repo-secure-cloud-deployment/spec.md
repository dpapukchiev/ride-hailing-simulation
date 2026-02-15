# public-repo-secure-cloud-deployment Specification

## Purpose
TBD - created by archiving change rust-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
### Requirement: No secrets committed to repository
The system SHALL source cloud credentials and sensitive configuration from environment variables or managed secret stores, and MUST NOT require committed plaintext secrets.

#### Scenario: Deployment with env-provided secrets
- **WHEN** a deployment pipeline runs for the serverless sweep stack
- **THEN** sensitive values are resolved from external secret sources and no secret files are required in repository contents

### Requirement: Least-privilege execution for orchestration and workers
The system SHALL assign IAM permissions scoped to minimum required actions for parent and worker runtimes.

#### Scenario: Worker access is restricted to run-scoped storage operations
- **WHEN** a worker executes a shard
- **THEN** its role can read required input and write only the configured output prefixes needed for that run

### Requirement: Local deploy flow uses temporary AWS sessions
The system SHALL provide a local script that builds Rust Lambda artifacts, bundles deployment inputs, and deploys the stack only when a temporary AWS login session is active.

#### Scenario: Deploy script validates active temporary session
- **WHEN** an operator runs the local deploy script without a valid temporary AWS session
- **THEN** the script fails fast with guidance to run the AWS login command before retrying

#### Scenario: Deploy script performs build-bundle-deploy sequence
- **WHEN** an operator runs the local deploy script with a valid temporary AWS session
- **THEN** the script builds Rust binaries, packages Lambda zip artifacts, and executes deployment with those artifacts in one command flow

#### Scenario: Deploy script reuses valid local build cache on subsequent runs
- **GIVEN** a prior successful deploy produced runtime artifacts and matching build metadata
- **AND** deployment build inputs have not changed
- **WHEN** an operator reruns the local deploy script
- **THEN** the script skips unnecessary rebuild steps and reuses cached artifacts
- **AND** the script still proceeds with Terraform deployment using the cached runtime zip

#### Scenario: Operator forces deterministic rebuild
- **WHEN** an operator runs the local deploy script with force-rebuild enabled
- **THEN** the script ignores cache hits, rebuilds runtime artifacts, and refreshes cache metadata before deployment

### Requirement: Deployment uses environment-driven secret handling
The system MUST keep credentials and sensitive values out of source control and SHALL require runtime and deployment secrets to be supplied through managed environment configuration.

#### Scenario: Repository contains no embedded cloud secrets
- **WHEN** deployment assets and application configuration are reviewed
- **THEN** no plaintext cloud credentials or long-lived secret values are present in tracked files

### Requirement: Cloud permissions follow least privilege
The system SHALL define IAM permissions for parent and child Lambdas that grant only actions required for invocation, storage access, and analytics operations.

#### Scenario: Worker role is constrained to required resources
- **WHEN** the child Lambda executes with its assigned IAM role
- **THEN** it can access only the configured S3 paths and required service actions for shard processing

