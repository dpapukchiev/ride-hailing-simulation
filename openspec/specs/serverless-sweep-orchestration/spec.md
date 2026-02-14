# serverless-sweep-orchestration Specification

## Purpose
TBD - created by archiving change minimal-aws-serverless-parameter-sweep. Update Purpose after archive.
## Requirements
### Requirement: Parent lambda shards sweep requests
The system SHALL provide a parent Lambda entrypoint that accepts a sweep request, validates required sweep parameters, computes deterministic shard boundaries, and produces one child invocation payload per shard.

#### Scenario: Valid sweep request creates shard payloads
- **WHEN** the parent Lambda receives a well-formed sweep request with shard sizing inputs
- **THEN** it computes non-overlapping shard boundaries that cover the requested parameter space exactly once

#### Scenario: Invalid sweep request is rejected
- **WHEN** the parent Lambda receives a request missing required sweep configuration
- **THEN** it returns a validation failure and does not dispatch any child invocations

### Requirement: API gateway can invoke the parent lambda
The system SHALL provide an API Gateway endpoint that accepts sweep-run requests and invokes the parent Lambda with the validated request payload.

#### Scenario: Valid API request reaches parent lambda
- **WHEN** a client submits a valid sweep-run request to the API Gateway route
- **THEN** API Gateway invokes the parent Lambda with the expected payload contract

#### Scenario: Invalid API request is rejected at ingress
- **WHEN** a client submits a malformed or incomplete sweep-run request
- **THEN** API Gateway rejects the request and the parent Lambda is not invoked

### Requirement: Parent lambda dispatches workers asynchronously
The system SHALL invoke child worker Lambdas in asynchronous fire-and-forget mode so orchestration does not wait for shard completion.

#### Scenario: Parent dispatch does not block on worker completion
- **WHEN** the parent Lambda dispatches shard payloads to child workers
- **THEN** it records dispatch outcomes and completes without waiting for child execution results

