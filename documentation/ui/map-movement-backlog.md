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

2) Pre-render map layer to a texture
    - Render cached road polylines to an `egui` texture when invalidated.
    - Reuse that texture every frame (single `painter.image`).
    - Note the caching invalidation rules in `documentation/ui/driver-map.md` so the docs explain when textures refresh.

3) Reduce geometry load
   - Add simplification or segment cap (e.g., draw every Nth segment at higher zooms).
   - Optional: filter to major road classes if OSRM layer provides metadata.

4) Throttle tile requests
   - Keep current inflight cap and optionally add LRU cache + eviction.

### C) Verification
1) Run local CI
   - `./ci.sh`
2) Document changes
   - Capture how driver movement and cached rendering work in the UI docs (e.g., `documentation/ui/driver-map.md`).
   - Update the changelog/notes so future work references the new map behaviors.

## Acceptance Criteria
- Drivers stay on roads (no diagonal cuts) when routing is OSRM Berlin.
- Map background stays smooth; CPU use drops significantly compared to current state.
- Map only re-renders on bounds/zoom/tile changes, not every frame.

## Notes
- If OSRM route geometry is large, prefer polyline encoding and decode once per route.
- For UI performance, keep texture sizes bounded and re-render only when needed.
- Keep `documentation/ui/driver-map.md` up to date with any movement or rendering behaviour changes so the UI docs match the implemented experience.
