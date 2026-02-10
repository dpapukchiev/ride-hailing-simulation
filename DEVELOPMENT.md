# Development Story: Building a Ride-Hailing Simulation in Rust

## Overview

This project was built in approximately **one week** without prior Rust experience, demonstrating rapid learning, effective AI collaboration, and multi-disciplinary engineering skills. This document tells the story of how it came together.

## The Challenge

**Goal**: Build a high-fidelity discrete event simulation of a ride-hailing marketplace (inspired by Bolt.eu) that demonstrates:
- Advanced simulation techniques (ECS, discrete event scheduling)
- Spatial algorithms (H3 indexing, haversine distance)
- Real-time visualization
- Parallel experimentation framework
- Production-quality code standards

**Constraint**: Zero prior Rust experience.

**Approach**: Learn Rust while building, using AI agents strategically with strong architectural guidance and quality standards.

## Research & Architecture

Before writing a single line of code, extensive research was conducted to understand the domain and make informed technical decisions.

### Research Phase

1. **Marketplace Dynamics**: Deep dive into Bolt.eu's operating model
   - Commission structure and driver retention
   - Multi-service switching (rides vs. food delivery)
   - Price-sensitive markets (Eastern Europe, Africa)
   - "Wild Goose Chase" threshold in high-density markets

2. **Simulation Architecture**: Studied discrete event simulation patterns
   - ECS vs. Actor model trade-offs
   - Event-driven vs. time-stepped execution
   - Spatial indexing strategies (H3 vs. QuadTree)

3. **Technical Stack**: Evaluated Rust ecosystem
   - `bevy_ecs` vs. `hecs` for ECS
   - `h3o` for geospatial operations
   - `rayon` for parallelism
   - `egui` for visualization

**Deliverables**: Four comprehensive research documents in `research/`:
- `Bolt Simulation Architecture Mechanics.md`
- `Marketplace Simulation Blueprint.md`
- `Rust DES Technical One-Pager.md`
- `Rust Ride-Sharing Simulation Specification.md`

### Architecture Decisions

With research complete, the architecture was designed around these key decisions:

- **ECS over Actors**: For 50K+ agents, ECS provides better data locality and cache performance
- **Discrete Events**: Binary heap-based event queue for deterministic, efficient execution
- **Sequential Execution**: Single-threaded simulation for determinism; parallelism reserved for experiments
- **Workspace Structure**: Three crates (`sim_core`, `sim_experiments`, `sim_ui`) for separation of concerns

**Architecture Principles**:
- Trait-based design for extensibility (matching algorithms, distributions)
- Event-driven systems reacting to `CurrentEvent` resource
- Targeted events via `EventSubject` for efficient entity updates
- Comprehensive telemetry for analysis

## AI-Guided Development

### Strategy

The entire implementation was built using multiple AI agents with strategic prompting techniques, guided by architectural decisions and quality standards:

1. **Architectural Guidance**: Provided high-level structure, patterns, and design constraints
2. **Code Generation**: AI agents generated Rust code based on specifications and research
3. **Learning Through Review**: Reviewed AI-generated code to understand Rust idioms (ownership, borrowing, traits, error handling)
4. **Iterative Refinement**: Prompted for improvements, error handling, and optimizations

### Prompting Techniques

- **Step-by-step decomposition**: Break complex features into smaller, well-defined tasks
- **Context building**: Provide research docs and architecture decisions as context
- **Quality gates**: Insist on tests, documentation, and benchmarks for every feature
- **Error handling**: Request proper `Result` types, avoid `unwrap()` in production code
- **Performance awareness**: Ask for optimizations and profiling considerations

### What This Demonstrates

Working effectively with AI coding tools requires a different skill set than writing code directly:
- Understanding system architecture well enough to guide generation
- Reviewing generated code for correctness, style, and edge cases
- Knowing when to accept, refine, or reject AI suggestions
- Maintaining quality standards across AI-generated code
- Strategic decomposition of complex problems for AI consumption

## Quality Standards

Throughout development, strict quality standards were maintained:

1. **Testing**: Unit tests for all systems, end-to-end integration tests, load tests
2. **Documentation**: Inline docs with examples, comprehensive SPEC.md and CONFIG.md with formulas
3. **Benchmarks**: Criterion.rs benchmarks for performance tracking with baseline comparison
4. **Linting**: Clippy pedantic mode enabled, no-warnings policy enforced
5. **Code Review**: All AI-generated code reviewed for correctness and style
6. **CI**: GitHub Actions workflow running tests, clippy, formatting checks, examples, and benchmarks on every push
6. **Spec Sync**: SPEC.md updated with every behavioral change

## Performance Engineering

### Bottleneck Discovery

During scale testing, logging became a significant performance bottleneck at 10K+ agents. The decision to remove logging overhead demonstrates production-minded thinking about the trade-off between observability and performance in a single-machine simulation context.

### Scale Achieved

- **Single Simulation**: 10,000 riders, 7,000 drivers over a 4-hour simulation window
- **Event Throughput**: ~12,200 events/sec in release build (~280K events processed in ~23s wall-clock)
- **Memory**: Efficient ECS storage with minimal allocations, LRU caches for spatial computations
- **Deterministic**: Reproducible results with seeded RNG throughout

### Experimentation Limits

The parallel experimentation framework (using rayon) hits single-machine resource limits (memory/CPU) when running large parameter sweeps. The architecture is designed for distributed execution via a coordinator/worker model (documented in `crates/sim_experiments/README.md`), but implementation is deferred until scaling beyond a single machine becomes necessary. This demonstrates understanding of when to architect for future needs vs. when to optimize prematurely.

## Key Skills Demonstrated

### Rapid Learning
- Mastered Rust fundamentals (ownership, borrowing, traits, error handling) while building a complex system
- Learned ECS patterns, spatial algorithms, and discrete event simulation techniques
- Understood performance characteristics and optimization strategies in Rust

### AI Collaboration
- Effectively leveraged AI tools with strategic prompting and quality control
- Maintained strict quality standards while using AI for code generation
- Learned to guide AI toward production-quality code through iterative refinement

### Multi-Disciplinary Skills
- **Researcher**: Conducted deep domain research, wrote technical specifications
- **Software Architect**: Designed system structure, made trade-off decisions, defined component boundaries
- **Software Engineer**: Implemented complex systems, wrote tests, optimized performance
- **Data Engineer**: Built telemetry pipeline, Parquet export, snapshot system
- **Data Scientist**: Created experimentation framework, health scoring, metrics analysis

### Project Management
- Structured the project with clear phases and deliverables
- Maintained quality gates throughout development
- Delivered a complete, well-documented codebase

## What Makes This Special

1. **Speed**: Built in approximately one week without prior Rust experience
2. **Quality**: Production-quality code with comprehensive tests, docs, and benchmarks
3. **Scale**: Successfully tested at 10,000 riders / 7,000 drivers
4. **Completeness**: Full pipeline from research to architecture to implementation to visualization to experimentation
5. **Real-World Relevance**: Models actual ride-hailing marketplace dynamics (pricing, matching, surge, driver behavior)

## Technical Highlights

- **Zero unsafe code** in core simulation logic
- **Deterministic execution** with seeded RNG throughout
- **Comprehensive telemetry** with Parquet export for analytics
- **Parallel experimentation** framework (hits single-machine limits)
- **Real-time visualization** with interactive UI
- **Multiple matching algorithms** with trait-based extensibility
- **Clippy pedantic** with no-warnings policy

## Future Work

The project demonstrates understanding of distributed systems -- the experimentation framework architecture is designed for a coordinator/worker model when scaling beyond single-machine limits becomes necessary. Other potential enhancements:

- **Distributed experimentation**: Multi-machine parameter sweeps via HTTP-based task distribution
- **Real routing**: Integrate OSRM or similar for realistic road network movement
- **Advanced matching**: Opportunity cost, shadow pricing, driver-value weighting
- **Multi-service switching**: Driver behavior across multiple platforms (rides, food delivery)

## Conclusion

This project demonstrates that with strong architectural guidance, effective AI collaboration, and rigorous quality standards, it is possible to rapidly learn a new language while building a sophisticated system. The multi-disciplinary approach -- from research to implementation to data analysis -- showcases the ability to wear multiple hats and deliver end-to-end solutions.
