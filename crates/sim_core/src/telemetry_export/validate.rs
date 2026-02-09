use crate::telemetry::{TripSnapshot, TripState};

/// Validates that timestamps in a trip snapshot follow the funnel order:
/// requested_at ≤ matched_at ≤ pickup_at ≤ dropoff_at (for completed trips)
/// or requested_at ≤ matched_at ≤ cancelled_at (for cancelled trips)
/// Returns an error message if validation fails, None if valid.
pub fn validate_trip_timestamp_ordering(trip: &TripSnapshot) -> Option<String> {
    if trip.requested_at > trip.matched_at {
        return Some(format!(
            "Trip {}: requested_at ({}) > matched_at ({})",
            trip.entity.to_bits(),
            trip.requested_at,
            trip.matched_at
        ));
    }

    match trip.state {
        TripState::EnRoute => {
            if trip.pickup_at.is_some() {
                return Some(format!(
                    "Trip {} (EnRoute): pickup_at should be None",
                    trip.entity.to_bits()
                ));
            }
            if trip.dropoff_at.is_some() {
                return Some(format!(
                    "Trip {} (EnRoute): dropoff_at should be None",
                    trip.entity.to_bits()
                ));
            }
            if trip.cancelled_at.is_some() {
                return Some(format!(
                    "Trip {} (EnRoute): cancelled_at should be None",
                    trip.entity.to_bits()
                ));
            }
        }
        TripState::OnTrip => {
            if let Some(pickup) = trip.pickup_at {
                if trip.matched_at > pickup {
                    return Some(format!(
                        "Trip {} (OnTrip): matched_at ({}) > pickup_at ({})",
                        trip.entity.to_bits(),
                        trip.matched_at,
                        pickup
                    ));
                }
            } else {
                return Some(format!(
                    "Trip {} (OnTrip): pickup_at should be Some",
                    trip.entity.to_bits()
                ));
            }
            if trip.dropoff_at.is_some() {
                return Some(format!(
                    "Trip {} (OnTrip): dropoff_at should be None",
                    trip.entity.to_bits()
                ));
            }
            if trip.cancelled_at.is_some() {
                return Some(format!(
                    "Trip {} (OnTrip): cancelled_at should be None",
                    trip.entity.to_bits()
                ));
            }
        }
        TripState::Completed => {
            if let Some(pickup) = trip.pickup_at {
                if trip.matched_at > pickup {
                    return Some(format!(
                        "Trip {} (Completed): matched_at ({}) > pickup_at ({})",
                        trip.entity.to_bits(),
                        trip.matched_at,
                        pickup
                    ));
                }
                if let Some(dropoff) = trip.dropoff_at {
                    if pickup > dropoff {
                        return Some(format!(
                            "Trip {} (Completed): pickup_at ({}) > dropoff_at ({})",
                            trip.entity.to_bits(),
                            pickup,
                            dropoff
                        ));
                    }
                } else {
                    return Some(format!(
                        "Trip {} (Completed): dropoff_at should be Some",
                        trip.entity.to_bits()
                    ));
                }
            } else {
                return Some(format!(
                    "Trip {} (Completed): pickup_at should be Some",
                    trip.entity.to_bits()
                ));
            }
            if trip.cancelled_at.is_some() {
                return Some(format!(
                    "Trip {} (Completed): cancelled_at should be None",
                    trip.entity.to_bits()
                ));
            }
        }
        TripState::Cancelled => {
            if let Some(cancelled) = trip.cancelled_at {
                if trip.matched_at > cancelled {
                    return Some(format!(
                        "Trip {} (Cancelled): matched_at ({}) > cancelled_at ({})",
                        trip.entity.to_bits(),
                        trip.matched_at,
                        cancelled
                    ));
                }
                if let Some(pickup) = trip.pickup_at {
                    if pickup > cancelled {
                        return Some(format!(
                            "Trip {} (Cancelled): pickup_at ({}) > cancelled_at ({})",
                            trip.entity.to_bits(),
                            pickup,
                            cancelled
                        ));
                    }
                }
            } else {
                return Some(format!(
                    "Trip {} (Cancelled): cancelled_at should be Some",
                    trip.entity.to_bits()
                ));
            }
            if trip.dropoff_at.is_some() {
                return Some(format!(
                    "Trip {} (Cancelled): dropoff_at should be None",
                    trip.entity.to_bits()
                ));
            }
        }
    }

    None
}
