## MODIFIED Requirements

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
