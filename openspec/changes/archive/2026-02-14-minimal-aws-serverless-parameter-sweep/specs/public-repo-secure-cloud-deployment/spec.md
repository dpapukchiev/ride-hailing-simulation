## ADDED Requirements

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
