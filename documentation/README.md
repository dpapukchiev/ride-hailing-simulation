# Documentation

This folder contains per-topic documentation for the ride-hailing simulation.
Each subfolder has:

- **`spec.md`** — technical specification for that topic (modules, systems, components)
- **`user-stories.md`** — small, implementable user stories with status tracking

## Topics

| Folder | Description |
|---|---|
| [core-sim](./core-sim/) | System flow, clock, ECS components, runner, spawner, scenario, distributions, movement, trip lifecycle, profiling |
| [drivers](./drivers/) | Driver decision system, off-duty checks, driver ECS components |
| [riders](./riders/) | Quote decision, accept/reject, cancellation, pickup ETA, rider ECS components |
| [matching](./matching/) | Matching algorithms (Simple, Cost-based, Hungarian), per-rider and batch matching systems |
| [pricing](./pricing/) | Pricing module, show-quote system, surge pricing |
| [telemetry](./telemetry/) | Telemetry resources, Parquet export, snapshot system |
| [experiments](./experiments/) | Parallel experimentation framework, parameter sweeps, health scores, and AWS serverless sweep operations |
| [ui](./ui/) | Native simulation UI, controls, charts, trip table |
| [testing](./testing/) | Unit tests, benchmarks (Criterion), load tests |
| [project](./project/) | Workspace layout, dependencies, tooling, task runner, CI |

## User Story Status Values

- **Done**: implemented in the codebase today
- **Backlog**: not implemented yet
- **In Progress**: actively being worked on

## Conventions

- Keep stories small and testable.
- Update status when a story is completed.
- Update `spec.md` whenever code changes alter behavior, structure, or workflows.
