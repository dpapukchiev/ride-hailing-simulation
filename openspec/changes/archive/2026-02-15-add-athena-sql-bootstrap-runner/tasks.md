## 1. Athena Bootstrap Runner

- [x] 1.1 Add a local script that executes Athena SQL statements in deterministic sequence and waits for each query to finish.
- [x] 1.2 Add explicit failure handling with query execution IDs and state-change reason output.
- [x] 1.3 Add placeholder substitution support for environment-specific values (`results bucket`, `results prefix`, `database`).

## 2. Ordered Pipeline Configuration

- [x] 2.1 Add a manifest file listing SQL files in execution order for “data layer ready” bootstrap.
- [x] 2.2 Support comments/blank lines in the manifest for maintainability.

## 3. Operator Documentation

- [x] 3.1 Update AWS serverless sweep docs with the one-command bootstrap example.
- [x] 3.2 Document how to add/reorder SQL steps by editing the manifest.

## 4. Validation

- [x] 4.1 Run script help/dry-run checks locally.
- [x] 4.2 Run formatting/lint checks relevant to changed files.
