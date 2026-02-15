## 1. Run context metadata export

- [x] 1.1 Add a versioned run-context metadata model in orchestration code with required analytics fields (run identity, shard plan summary, request provenance, config fingerprint).
- [x] 1.2 Persist one run-context metadata object to S3 for each accepted API-triggered run using deterministic object keys.
- [x] 1.3 Ensure validation failures do not emit run-context records and add coverage for accepted vs rejected request behavior.

## 2. Per-point effective parameter export

- [x] 2.1 Add worker-side serialization for effective simulation parameters after point resolution.
- [x] 2.2 Persist per-point effective-parameter records in Athena-queryable format with deterministic join keys (`run_id`, `shard_id`, `point_index`) and parameter fingerprint.
- [x] 2.3 Enforce idempotent writes for retries so duplicate run/shard/point exports do not break joins.

## 3. Athena schema and query contract updates

- [x] 3.1 Add or update Athena SQL definitions for run-context and effective-parameter datasets under existing bootstrap workflow.
- [x] 3.2 Ensure partition and schema conventions align with existing outcomes datasets for efficient joins.
- [x] 3.3 Add example queries or runbook snippets demonstrating joins from outcomes to run context and effective parameters.

## 4. Validation and documentation

- [x] 4.1 Add integration coverage that verifies S3 object layout and join-key consistency across outcomes, run context, and parameter exports.
- [x] 4.2 Add analytics-level validation (Athena smoke query or equivalent) proving outcomes can be joined to configuration for a sample run.
- [x] 4.3 Update relevant documentation to describe new dataset fields, schema versioning expectations, and partial-run caveats.
