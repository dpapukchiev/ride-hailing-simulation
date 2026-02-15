## 1. Diagram Structure and Tooling

- [x] 1.1 Create a `documentation/diagrams/` layout with separate `src` and `rendered` directories.
- [x] 1.2 Add a repository render script/command that generates all diagram outputs from source files deterministically.
- [x] 1.3 Add contributor documentation for diagram authoring, rendering, and commit expectations.

## 2. Baseline Diagram Set

- [x] 2.1 Add initial module-level architecture diagram source files for key system pieces.
- [x] 2.2 Add initial sequence diagram source files for core runtime flows.
- [x] 2.3 Generate and commit rendered SVG artifacts for all baseline diagrams.

## 3. CI Render Freshness Enforcement

- [x] 3.1 Add a CI step that runs the canonical diagram render command.
- [x] 3.2 Fail CI when regenerated diagram outputs differ from committed rendered artifacts.
- [x] 3.3 Provide actionable CI failure messaging that tells contributors how to regenerate and commit diagrams.

## 4. Validation

- [x] 4.1 Run local diagram render command and verify no diff on a second run.
- [x] 4.2 Run repository checks relevant to changed files (format/lint/workflow validation where applicable).
