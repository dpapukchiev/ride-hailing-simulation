const DEFAULT_FIRST_RADIUS_M: f64 = 30.0;
const DEFAULT_ADDITIONAL_RADIUS_M: f64 = 15.0;
const FALLBACK_FIRST_RADIUS_M: f64 = 45.0;
const FALLBACK_ADDITIONAL_RADIUS_M: f64 = 30.0;

pub(super) fn encode_radiuses(count: usize, radiuses: &[f64]) -> String {
    let default = radiuses
        .last()
        .copied()
        .unwrap_or(DEFAULT_ADDITIONAL_RADIUS_M)
        .max(0.0);

    (0..count)
        .map(|idx| {
            let radius = radiuses.get(idx).copied().unwrap_or(default).max(0.0);
            format!("{:.1}", radius)
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub(super) fn default_radiuses(count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let mut radiuses = Vec::with_capacity(count);
    radiuses.push(DEFAULT_FIRST_RADIUS_M);
    radiuses.extend(std::iter::repeat_n(DEFAULT_ADDITIONAL_RADIUS_M, count - 1));
    radiuses
}

pub(crate) fn fallback_radiuses(count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let mut radiuses = Vec::with_capacity(count);
    radiuses.push(FALLBACK_FIRST_RADIUS_M);
    radiuses.extend(std::iter::repeat_n(FALLBACK_ADDITIONAL_RADIUS_M, count - 1));
    radiuses
}

pub(crate) fn radiuses_for_attempt(attempt: usize, count: usize) -> Vec<f64> {
    if attempt == 0 {
        default_radiuses(count)
    } else {
        fallback_radiuses(count)
    }
}
