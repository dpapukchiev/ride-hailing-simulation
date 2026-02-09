use std::cmp::Ordering;

use super::response::{OsrmMatching, OsrmTracepoint};

pub(super) fn select_best_matching<'a>(
    matchings: &'a [OsrmMatching],
    tracepoints: &[Option<OsrmTracepoint>],
) -> Option<(usize, &'a OsrmMatching)> {
    let mut best: Option<(usize, &OsrmMatching)> = None;

    for (idx, matching) in matchings.iter().enumerate() {
        best = Some(if let Some((best_idx, best_matching)) = best {
            match matching
                .confidence
                .partial_cmp(&best_matching.confidence)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => (idx, matching),
                Ordering::Less => (best_idx, best_matching),
                Ordering::Equal => {
                    let best_tp =
                        first_tracepoint_index(tracepoints, best_idx).unwrap_or(usize::MAX);
                    let next_tp = first_tracepoint_index(tracepoints, idx).unwrap_or(usize::MAX);
                    if next_tp < best_tp {
                        (idx, matching)
                    } else {
                        (best_idx, best_matching)
                    }
                }
            }
        } else {
            (idx, matching)
        });
    }

    best
}

pub(super) fn first_tracepoint_index(
    tracepoints: &[Option<OsrmTracepoint>],
    matching_index: usize,
) -> Option<usize> {
    tracepoints
        .iter()
        .enumerate()
        .find(|(_, trace)| trace.as_ref().and_then(|tp| tp.matching_index) == Some(matching_index))
        .map(|(idx, _)| idx)
}
