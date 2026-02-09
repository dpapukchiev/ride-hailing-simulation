use h3o::CellIndex;
use sim_core::routing::{
    build_route_provider, H3GridRouteProvider, RouteProvider, RouteProviderKind,
};

#[test]
fn h3_grid_provider_returns_route() {
    let provider = H3GridRouteProvider;
    let origin = CellIndex::try_from(0x8a1fb46622dffff_u64).expect("valid cell");
    let neighbor = origin
        .grid_disk::<Vec<_>>(3)
        .into_iter()
        .find(|c| *c != origin)
        .expect("neighbor");

    let route = provider.route(origin, neighbor).expect("route");
    assert!(route.distance_km > 0.0);
    assert!(route.duration_secs > 0.0);
    assert!(!route.cells.is_empty());
}

#[test]
fn h3_grid_provider_same_cell_returns_some() {
    let provider = H3GridRouteProvider;
    let cell = CellIndex::try_from(0x8a1fb46622dffff_u64).expect("valid cell");
    assert!(provider.route(cell, cell).is_some());
}

#[test]
fn route_provider_kind_default_is_h3() {
    assert_eq!(RouteProviderKind::default(), RouteProviderKind::H3Grid);
}

#[test]
fn build_route_provider_h3grid() {
    let provider = build_route_provider(&RouteProviderKind::H3Grid);
    let cell = CellIndex::try_from(0x8a1fb46622dffff_u64).expect("valid cell");
    let neighbor = cell
        .grid_disk::<Vec<_>>(2)
        .into_iter()
        .find(|c| *c != cell)
        .expect("neighbor");
    assert!(provider.route(cell, neighbor).is_some());
}
