## 1. Cache-aware Deploy Script

- [x] 1.1 Add cache fingerprint generation for deploy build inputs (target/profile/image + source hash).
- [x] 1.2 Remove unconditional target cleanup and gate packaging on cache hit/miss.
- [x] 1.3 Add `--force-rebuild` (and env equivalent) to bypass cache checks.
- [x] 1.4 Add structured logging for cache decisions and stage durations.

## 2. Faster Containerized Build Environment

- [x] 2.1 Add a reusable builder image that pre-installs native build dependencies.
- [x] 2.2 Add Docker pull policy control (`always`, `if-missing`, `never`) with safe default.
- [x] 2.3 Ensure Windows/MSYS path conversion handling remains intact.

## 3. Documentation & Operator UX

- [x] 3.1 Update serverless deployment docs with incremental deploy behavior.
- [x] 3.2 Document new flags/env vars and when to use force rebuild.
- [x] 3.3 Add troubleshooting notes for stale cache and cleanup workflow.

## 4. Validation

- [x] 4.1 Run script shell syntax and lint checks.
- [x] 4.2 Run one cold deploy build and one hot repeat run; capture timing deltas.
- [x] 4.3 Confirm Terraform apply path still receives correct runtime artifact input.
