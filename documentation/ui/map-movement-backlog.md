# Map Movement + Map Rendering Backlog

## Goals
- Drivers follow real OSRM route geometry (no diagonal shortcuts).
- Map background does not fully re-render every frame.

## Scope
- Real movement: use OSRM route polyline for driver movement and rendering.
- Cached map: draw tiles/roads once per change, then reuse.

## Dependencies
- OSRM route geometry available via route API (geometry polyline or GeoJSON).
- OSRM tile service available (already running).

## Work Items

### A) Real movement on route geometry
1) Add route geometry to routing output in sim_core
    - Update OSRM client to request `geometries=geojson` (or polyline) for route calls.
    - Extend route result struct to include the polyline coordinates (lat/lng list).
    - Wire through any caching layer so geometry is preserved per route.

2) Store per-trip route path in sim state
    - Add route polyline to trip or driver state (e.g., current route segment list).
    - Update serialization/telemetry snapshots to expose the route geometry to sim_ui.
    - Evaluate whether per-trip route segments should live on the Trip entity alone or be split into dedicated components (e.g., `RouteSegment`, `GeoPosition`) for reuse by other systems.

3) Move drivers along route geometry
   - Replace straight-line interpolation with path traversal.
   - Track current segment index + distance along segment.
   - Convert speed/time to distance traveled per tick and advance along segments.
   - Handle edge cases: zero-length segments, short routes, rerouting mid-trip.

4) Render using route geometry
    - Draw driver position from path traversal (not interpolated endpoints).
    - Optional: render active route polyline overlay for debugging.
    - Capture any rendering tweaks or overlays in `documentation/ui/driver-map.md` so the UI docs describe how the driver route and geometrics behave.

### B) Avoid re-drawing the map every frame
1) Cache projected tile geometry
    - Cache lat/lng -> screen projection per tile for current map bounds + zoom.
    - Invalidate only when bounds/zoom change or tile geometry updates.
    - Build the cache incrementally during frames so the UI never blocks when opening the map.
    - Store simplified projected paths (e.g., drop points closer than a pixel or cap segments per tile) before caching to keep redraw cost bounded.

2) Pre-render map layer to a texture
    - Render cached road polylines to an `egui` texture when invalidated.
    - Reuse that texture every frame (single `painter.image`).
    - Note the caching invalidation rules in `documentation/ui/driver-map.md` so the docs explain when textures refresh.

3) Reduce geometry load
    - Add simplification or segment cap (e.g., draw every Nth segment at higher zooms).
    - Optional: filter to major road classes if OSRM layer provides metadata.

4) Protect the main thread from tile decoding bursts
    - Keep tile request/decode concurrency low during initial load (e.g., reduce inflight cap while map warms up).
    - Queue rebuild work across frames and only render projects/line buffers once ready.

4) Throttle tile requests
    - Keep current inflight cap and optionally add LRU cache + eviction.

5) Document incremental caching behavior
    - Capture these new cache + simplification rules in `documentation/ui/driver-map.md` so readers understand how and when map redraws occur.

### C) Verification
1) Run local CI
    - `./ci.sh`
2) Document changes
    - Capture how driver movement and cached rendering work in the UI docs (e.g., `documentation/ui/driver-map.md`).
    - Update the changelog/notes so future work references the new map behaviors.

### Next start marker
- Next agent should pick up here: implement the incremental projection cache + threaded rebuild so opening the map no longer blocks the UI. Focus on layout/tile invalidation logic before tackling texture baking.

### D) Incremental projection cache + threaded rebuild
1) Build incremental projection cache
    - Track visible tiles, zoom, and bounds of the UI view.
    - Cache lat/lng -> screen projection for every vertex in a tile when the tile is first drawn.
    - Invalidate projections only when zoom/bounds change or when raw tile geometry is refreshed.
    - Emit lightweight invalidation markers that allow the UI thread to skip rebuilding tiles that already have valid projections.

2) Pipe rebuild work through background threads
    - Offload tile decoding and projected path simplification to worker threads so the UI thread only schedules renders.
    - Enforce a low inflight cap while the map is warming up; progressively increase it once the cache is primed.
    - Collect rebuild results across frames and signal the UI thread once new buffers are ready for texture baking.
    - Provide hooks for cancellation when bounds/zoom change before a tile finishes decoding.

3) Coordinate layout and invalidation logic
    - Record which tiles depend on which cached paths so the renderer knows when to redraw.
    - When tiles move offscreen, keep projections for a short grace period to avoid churn as the user pans slightly.
    - Ensure thread-safety when bumping invalidation counters or swapping cached buffers.
    - Capture this behavior in `documentation/ui/driver-map.md` so the docs describe when tiles rebuild versus when cached textures stay put.

## Acceptance Criteria
- Drivers stay on roads (no diagonal cuts) when routing is OSRM Berlin.
- Map background stays smooth; CPU use drops significantly compared to current state.
- Map only re-renders on bounds/zoom/tile changes, not every frame.

## Notes
- If OSRM route geometry is large, prefer polyline encoding and decode once per route.
- For UI performance, keep texture sizes bounded and re-render only when needed.
- Keep `documentation/ui/driver-map.md` up to date with any movement or rendering behaviour changes so the UI docs match the implemented experience.
