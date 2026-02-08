# Driver Map Reference

This file describes how the simulation UI renders drivers and their routes so that
designers, reviewers, and future contributors understand the assumptions around map
movement and cached rendering. Update this document every time the driver map
behaviour changes (geometry, projection, caching, controls, legends, etc.).

## Overview
- The driver map sits at the heart of `sim_ui` and is layered beneath the control
  panel and metric overlays. It includes a tile-based background, stateful driver
  and rider icons, and optional debug overlays (grid, stats, route lines).
- Drivers are represented with `D` or `D(R)` labels depending on whether a rider is
  assigned; the labels can optionally include earned/target and fatigue metrics.
- Control inputs (map size, zoom, simulation speed) are exposed via the UI chrome
  so map playback stays in sync with user-adjustable parameters.

## Geometry and Movement
- Driver positions are now derived from OSRM route geometry instead of straight
  line interpolation, so the map provenance is tied to real-world roads (Berlin
  traffic profile by default). Document the sources of truth for location data:
  the route polyline stored per trip, the segment index/distance bookkeeping, and
  how speed/time convert to distance per tick.
- Note which OSRM query parameters are required (`geometries=geojson` or
  polyline) and how the response is cached to prevent redundant routing work.
- Track edge cases such as zero-length segments, short reroutes, and rerouting in
  progress. Explain how they are surfaced (or hidden) by the UI and how doc tests
  should cover them.

## Rendering Strategy
- The cached map background prevents a full re-render each frame. Describe the
  tile projection cache, the criteria that invalidate it (bounds, zoom, geometry
  updates), and how caches map to `egui` textures.
- Document any simplification/sampling rules that reduce geometry density at a
  given zoom level and how optional filters (major road classes, metadata) should
  influence the drawn streets.
- Mention throttling of tile/geometry fetches, inflight caps, and cache eviction
  so anyone tuning performance understands the current behaviour.

## Documentation Expectations
- When driver movement or cached rendering behaviour changes, update this file and
  reference the change in `documentation/ui/map-movement-backlog.md` so future work
  knows the new baseline.
- Include screenshots, diagrams, or sequence descriptions as needed to clarify why
  OSRM geometry and cached textures were chosen.
- Link to telemetry or CLI commands (`./ci.sh`, `cargo xtask ui`, etc.) that prove
  the map remains smooth and drivers stay on the road.
