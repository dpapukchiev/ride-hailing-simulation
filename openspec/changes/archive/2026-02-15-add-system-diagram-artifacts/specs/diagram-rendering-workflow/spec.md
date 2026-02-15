## ADDED Requirements

### Requirement: Diagrams can be rendered locally from source
The system SHALL provide a documented local command that renders all diagram source files into committed output artifacts.

#### Scenario: Contributor regenerates all diagrams locally
- **WHEN** a contributor runs the documented render command
- **THEN** all diagram sources are rendered into the expected output directory
- **AND** the command exits non-zero if rendering fails for any diagram

### Requirement: Render outputs are GitHub-viewable
The system SHALL generate diagram outputs in formats that are directly viewable in GitHub file and markdown views.

#### Scenario: Reviewer opens rendered output on GitHub
- **WHEN** a reviewer opens a rendered diagram artifact in the repository
- **THEN** the diagram is visible without requiring external rendering services or proprietary tools

### Requirement: Rendering workflow is deterministic
The system SHALL produce deterministic rendered output from the same source inputs to reduce noisy diffs.

#### Scenario: Contributor reruns render without source changes
- **GIVEN** diagram source files and renderer version are unchanged
- **WHEN** the contributor reruns the render command
- **THEN** rendered artifacts remain unchanged

### Requirement: CI rejects unrendered diagram source changes
The system SHALL run a CI validation that fails when committed diagram source files are not matched by up-to-date rendered artifacts.

#### Scenario: Pull request changes diagram source without regenerated renders
- **WHEN** a pull request modifies diagram source files and rendered outputs are stale or missing
- **THEN** the CI validation fails
- **AND** the failure message instructs contributors to run the local render command and commit generated artifacts

### Requirement: Documentation defines authoring and regeneration workflow
The system SHALL provide contributor-facing documentation that explains how to add, update, and regenerate diagrams.

#### Scenario: Contributor adds a new key system diagram
- **WHEN** a contributor follows the diagrams documentation
- **THEN** they can author a source diagram, run the local render process, and commit both source and rendered outputs correctly
