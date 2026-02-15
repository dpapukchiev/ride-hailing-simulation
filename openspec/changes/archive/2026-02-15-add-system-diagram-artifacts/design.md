## Context

The proposal introduces architecture diagrams as versioned artifacts and requires both source and rendered outputs to be committed. The repository currently lacks a standard diagrams location, a render pipeline, and baseline diagrams for core system flows. The solution must work for local contributors, produce deterministic output that can be reviewed in pull requests, and keep diagrams directly viewable in GitHub without external hosting.

## Goals / Non-Goals

**Goals:**
- Add a stable folder structure that separates diagram source from rendered assets while keeping both in version control.
- Standardize on one local rendering command that generates all diagrams consistently.
- Ship an initial baseline set of module and runtime sequence diagrams for key system pieces.
- Ensure generated diagram files are directly viewable in GitHub and easy to diff in PRs.

**Non-Goals:**
- Full architecture coverage for every module in the first iteration.
- Automated extraction of diagrams from Rust code.
- Replacing narrative architecture docs in `README.md`, `SPEC.md`, or `documentation/`.

## Decisions

### Decision: Use Mermaid source files and generated SVG artifacts

Store authoring sources as Mermaid text files and generate SVG render outputs locally.

**Why this choice**
- Mermaid source is plain text, easy to review, and familiar to GitHub users.
- SVG is web-native, renders directly in GitHub file view, and remains easy to embed from markdown docs.
- A single syntax family can cover both module interaction and runtime flow sequence views.

### Decision: Adopt a docs-first folder layout under `documentation/diagrams/`

Use a structure such as:
- `documentation/diagrams/src/` for diagram source
- `documentation/diagrams/rendered/` for generated SVG output
- `documentation/diagrams/README.md` for conventions and regeneration commands

**Why this choice**
- Keeps architecture artifacts near existing documentation instead of mixing with crate code.
- Makes generated outputs discoverable and avoids ad hoc output locations.
- Provides a clear contract for contributors and CI checks.

### Decision: Provide one repository command to render all diagrams

Add a single command (script or task runner entry) that regenerates every diagram from `src/` into `rendered/` in deterministic order.

**Why this choice**
- Reduces contributor friction and command drift.
- Prevents partial updates where source changes are committed without refreshed rendered assets.
- Simplifies future CI validation that rendered output matches source.

### Decision: Enforce rendered freshness in CI

Add a CI job step that runs the canonical render command and fails if it produces a diff in committed rendered artifacts.

**Why this choice**
- Prevents stale rendered diagrams from being merged.
- Keeps GitHub-visible outputs synchronized with diagram source changes.
- Gives contributors immediate feedback when they forget to regenerate diagrams.

### Decision: Seed a baseline diagram set for key system pieces

Include initial diagrams in this change rather than deferring to follow-up work:
- Module interaction sequence views for local and serverless runtime boundaries.
- Sequence diagrams for core flows (e.g., simulation run execution and results handling).

**Why this choice**
- Delivers immediate documentation value.
- Demonstrates the expected authoring style and repository conventions.
- Creates concrete examples for future contributors to copy and extend.

## Alternatives Considered

1. PlantUML + C4-PlantUML
   - Pros: mature architecture modeling ecosystem.
   - Cons: heavier local setup and weaker default GitHub-native rendering path.

2. Structurizr DSL + external renderer
   - Pros: strong architecture modeling semantics.
   - Cons: less straightforward for sequence diagrams and higher onboarding/tooling overhead.

3. Manual drawing tools with exported images only
   - Pros: quick initial visuals.
   - Cons: no diagrams-as-code source, poor maintainability, and difficult PR review.

## Risks / Trade-offs

- [Complex architecture diagrams can become visually dense] -> Prefer module-level sequence layouts that keep lane spacing readable and easy to review.
- [Rendered SVG drift from source if contributors forget regeneration] -> Add clear contribution docs and a lightweight validation step in local/CI workflows.
- [Diagram sprawl and inconsistency over time] -> Define naming/layout conventions and maintain an index in `documentation/diagrams/README.md`.

## Migration Plan

1. Create the diagrams directory layout and contribution/readme guidance.
2. Add Mermaid tooling plus a single render-all command for local generation.
3. Author and commit the baseline module and runtime sequence diagram source files.
4. Generate and commit SVG artifacts, then link them from documentation pages.
5. Add a verification step (script/CI check) to ensure rendered outputs are current.

## Open Questions

- Which exact key flows should be mandatory in the initial sequence set (minimum required list)?
- Should render verification run in the main CI pipeline immediately or start as a local-only check first?
