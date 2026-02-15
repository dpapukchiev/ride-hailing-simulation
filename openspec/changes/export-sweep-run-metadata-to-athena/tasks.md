## 1. Run context metadata export

- [ ] 1.1 Add a versioned run-context metadata model in orchestration code with required analytics fields (run identity, shard plan summary, request provenance, config fingerprint).
- [ ] 1.2 Persist one run-context metadata object to S3 for each accepted API-triggered run using deterministic object keys.
- [ ] 1.3 Ensure validation failures do not emit run-context records and add coverage for accepted vs rejected request behavior.

## 2. Per-point effective parameter export

- [ ] 2.1 Add worker-side serialization for effective simulation parameters after point resolution.
- [ ] 2.2 Persist per-point effective-parameter records in Athena-queryable format with deterministic join keys (`run_id`, `shard_id`, `point_index`) and parameter fingerprint.
- [ ] 2.3 Enforce idempotent writes for retries so duplicate run/shard/point exports do not break joins.

## 3. Athena schema and query contract updates

- [ ] 3.1 Add or update Athena SQL definitions for run-context and effective-parameter datasets under existing bootstrap workflow.
- [ ] 3.2 Ensure partition and schema conventions align with existing outcomes datasets for efficient joins.
- [ ] 3.3 Add example queries or runbook snippets demonstrating joins from outcomes to run context and effective parameters.

## 4. Validation and documentation

- [ ] 4.1 Add integration coverage that verifies S3 object layout and join-key consistency across outcomes, run context, and parameter exports.
- [ ] 4.2 Add analytics-level validation (Athena smoke query or equivalent) proving outcomes can be joined to configuration for a sample run.
- [ ] 4.3 Update relevant documentation to describe new dataset fields, schema versioning expectations, and partial-run caveats.
