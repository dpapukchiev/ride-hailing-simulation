# AGENTS Guide

This file is the concise reference any AI collaborator or future maintainers should scan once before editing. It keeps token usage lower by capturing the essentials (architecture, testing, workflow), avoiding repeated repo exploration.

## Project Snapshot
- **Language & stack**: Rust workspace with `crates/sim_core` (engine), `crates/sim_experiments` (parallel parameter sweeps), `crates/sim_ui` (egui-based visualization). `xtask/` and `infra/osrm/` host tooling and OSRM setup respectively.
- **Key docs**: `README.md` (high-level), `SPEC.md` (system spec), `CONFIG.md` (config parameters), `DEVELOPMENT.md` (narrative), `documentation/` (per-topic deep dives).
- **Entry points**: `cargo run -p sim_core --example scenario_run[_large]`, `cargo run -p sim_ui`, and experiment runners under `sim_experiments`.

## Common commands
- `./ci.sh` - to apply the same checks as CI (`check` by default)
- `cargo fmt --all`, `cargo clippy --all -- -D warnings` before merge PRs.
- `cargo test -p sim_core` (core) or `cargo test` for smaller scopes.
- `cargo run -p sim_core --example scenario_run_large --release` for performance benchmarks; attach perf outputs to docs if needed.
- `cargo run -p xtask -- --help` to inspect available automation tasks.

## Collaboration workflow
1. **Minimal context**: Tell the agent the goal and the files that may change; include snippets/diffs only when necessary. Avoid dumping entire modules.
2. **Scoping**: Append a little note like `Allowed paths: crates/sim_core/src/matching/**/*; Do not touch: infra/`. Explicit boundaries prevent extra context.
3. **Prompt template**:
   ```text
   Task: <goal>
   Allowed files: <list>
   Avoid files: <list>
   Constraints: <tests/compat/formatting>
   Acceptance: run `cargo test ...`, ensure lint passes
   Output: brief summary + justification for key decisions
   ```
4. **Repeatable units**: Aim for edits that touch one crate or UI area at a time to keep files short and context local.

## Efficiency & token savings
- Keep files focused (split large systems into modules); the agent will need to load less context.
- Favor explicit contracts (types/interfaces) so autocomplete decisions require fewer surrounding lines.
- Mention failing commands or test errors instead of whole logs (first few lines + stack frame).
- Reset sessions frequently; start a new conversation when the task changes significantly.

## When to mention this guide
- Reference `AGENTS.md` once at the start of each new AI-driven task or agent session (e.g., “See AGENTS.md for repo constraints and command expectations”). You do not need to quote it each turn; just mention it to signal the agent should re-read the key rules.
- If you widen the scope (new crate, infra, UI), update this file with the new constraints and rerun the “read once” step.

## Supporting docs
- `DEVELOPMENT.md` explains the development story and experimentation philosophy.
- `documentation/` is split by topic—use the relevant subdirectory instead of asking the agent to re-derive the behavior.
- `SPEC.md` and `CONFIG.md` contain system requirements and knobs; reference them before adjusting matching/pricing logic.

Keeping this file short, accurate, and up to date eliminates wasted token spend. If nothing else changes, just refer to it—no extra refactor needed right now.
