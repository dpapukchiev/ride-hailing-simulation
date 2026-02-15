## Context

The API sweep flow already accepts orchestration requests, fans out shard work, and persists shard outcomes for Athena analysis. However, analytics consumers still lack two durable datasets required for reliable attribution: (1) run-level context metadata that describes how a run was planned and triggered, and (2) per-point effective simulation parameters that capture the concrete values executed for each point.

Without these datasets, analysts must infer configuration from request payload fragments or reconstruct parameters from code defaults, which is error-prone and blocks repeatable comparisons across runs.

## Goals / Non-Goals

**Goals:**
- Export a single run-context metadata record per API-triggered run with deterministic identifiers and configuration fingerprint fields.
- Export one effective-parameter record per executed point so outcomes can be joined to concrete simulation inputs.
- Define deterministic join keys and partition conventions shared by outcomes, run context, and parameter exports.
- Keep Athena access straightforward by extending existing SQL/table conventions instead of introducing a separate analytics pipeline.

**Non-Goals:**
- Building a full lineage catalog or data governance platform.
- Backfilling historical runs that were produced before this contract exists.
- Replacing existing outcomes datasets or changing core outcome semantics.

## Decisions

### 1) Persist run context at orchestration acceptance time
- **Decision:** When an API request is accepted, write a run-context metadata object that includes at least `run_id`, `run_date`, schema/version, request source, shard planning inputs, shard plan summary, and a stable configuration fingerprint.
- **Rationale:** This ensures every accepted run has a canonical metadata anchor even before workers complete, and avoids reconstructing run intent from scattered payload snapshots.
- **Alternatives considered:**
  - Persist run context only after all shards finish: simpler completeness semantics, but no metadata for failed/partial runs and weaker observability.
  - Store context only in logs: low effort, but not join-friendly or query-efficient.

### 2) Emit per-point effective parameters as a first-class analytics dataset
- **Decision:** During shard execution, emit records keyed by run and point identity that contain the fully resolved parameter set actually used by simulation code.
- **Rationale:** Outcomes become directly attributable to concrete effective inputs, including defaults/derived values, not just high-level sweep dimensions.
- **Alternatives considered:**
  - Store only high-level sweep dimensions: smaller payload, but does not capture resolved defaults or derived fields.
  - Embed parameter blobs in outcome rows: simpler writes, but duplicates data and increases scan cost for outcome-only queries.

### 3) Standardize deterministic join keys across datasets
- **Decision:** Require shared keys (`run_id`, `shard_id`, `point_index`) and a stable per-point parameter hash/fingerprint field to support strict joins and dedupe validation.
- **Rationale:** A fixed join contract removes ambiguity and lets analysts safely join outcomes to context/config across partial reruns and retries.
- **Alternatives considered:**
  - Join only by `run_id`: too coarse for per-point analytics.
  - Use implicit ordering joins: brittle and non-deterministic under retries.

### 4) Extend Athena metadata in-place
- **Decision:** Update existing Athena SQL assets to include tables (or views) for run context and effective parameters, with partition conventions aligned to current run outputs.
- **Rationale:** Preserves operator workflow and avoids introducing a second metadata/bootstrap path.
- **Alternatives considered:**
  - Create a separate analytics database for metadata tables: cleaner isolation, but extra operator overhead and duplicated setup flow.

## Risks / Trade-offs

- [Parameter payload growth increases storage and query scan costs] -> Persist normalized fields plus a compact serialized parameter payload, and partition by run dimensions to limit scanned data.
- [Schema drift between runtime parameter structs and exported analytics schema] -> Version exported schemas and add contract tests that validate expected fields and join keys.
- [Retry/idempotency can create duplicate parameter rows] -> Use deterministic object keys and idempotent write behavior keyed by run/shard/point identity.
- [Partial run failures produce incomplete per-point datasets] -> Keep run-context record authoritative and include status fields so analytics can filter incomplete runs explicitly.

## Migration Plan

1. Add run-context metadata model and S3 writer in orchestration flow, including versioned schema fields and deterministic key construction.
2. Add per-point effective-parameter export in worker flow after effective parameter resolution and before/alongside outcome persistence.
3. Update Athena SQL definitions and bootstrap flow to register new datasets and joinable columns.
4. Add integration tests validating object layout, schema fields, and cross-dataset joinability.
5. Update runbooks/docs with example Athena joins from outcomes to run context and effective parameters.

Rollback strategy:
- Feature-gate new exports via configuration; if regressions appear, disable metadata exports while keeping existing outcome persistence intact.

## Open Questions

- Should effective-parameter records be fully columnar (explicit columns per field) or include a hybrid model with required columns plus a JSON map for forward compatibility?
