## Context

The repository already includes SQL artifacts for creating Athena/Glue metadata and repairing table partitions, but execution is manual. The user goal is “one command and Athena data layer is ready,” with easy extension and ordered workflows (lightweight orchestration without deploying Airflow/Temporal).

## Goals / Non-Goals

**Goals**
- Provide a single local command to execute the Athena setup pipeline end-to-end.
- Ensure deterministic SQL ordering that is easy to edit.
- Fail fast and clearly when an Athena statement fails.
- Keep deployment overhead zero (no always-on orchestrator service).

**Non-Goals**
- Building a general workflow engine.
- Replacing Athena analytical queries with materialized pipelines.
- Introducing managed workflow services for this setup step.

## Decision

### Decision: Use a local Python CLI runner + ordered manifest

Implement a script (`apply_athena_sql.py`) that:
1. Reads a text manifest describing SQL file order.
2. Applies environment substitutions.
3. Executes each statement through Athena APIs via AWS CLI.
4. Polls for completion and exits non-zero on any failure.

### Why this choice

- **Lowest operational burden**: no new infra/services to deploy.
- **Good enough orchestration semantics** for sequential DDL/repair tasks.
- **Easy extension**: adding/reordering SQL is manifest-only.
- **Portable**: works in local shells and CI with temporary credentials.

## Alternatives considered

1. **Airflow DAG (local or managed)**
   - Pros: richer scheduling/dependency graph.
   - Cons: overkill for a short sequential setup pipeline; requires extra deployment/runtime footprint.

2. **Temporal workflow (local dev server or cloud)**
   - Pros: durable workflows and retries.
   - Cons: substantial complexity and new moving parts for a bootstrap-only task.

3. **Terraform-only SQL execution resource pattern**
   - Pros: infra and query setup in one apply.
   - Cons: query evolution becomes awkward, limited run-time observability, and harder ad hoc reruns.

## Risks / Trade-offs

- AWS CLI dependency and active credentials are required.
- Polling-based execution is sequential and can be slower than parallel orchestration.
- Placeholder substitution must stay aligned with SQL templates.

## Migration Plan

1. Add manifest-driven runner and default readiness pipeline file.
2. Update docs to use one command for Athena setup.
3. Validate with dry-run and shell syntax checks.
