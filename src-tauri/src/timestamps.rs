//! UTC timestamps shared by stored conversations, shelves and search results.

use jiff::Timestamp;

pub fn format(timestamp: Timestamp) -> String {
    // Keep the numeric UTC offset used by existing records and their lexical sort.
    timestamp
        .display_with_offset(jiff::tz::Offset::UTC)
        .to_string()
}

pub fn now_string() -> String {
    format(Timestamp::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_offsets_and_fractional_timestamps_round_trip() {
        let legacy: Timestamp = "2026-08-15T14:00:00.123456+02:00".parse().unwrap();
        assert_eq!(format(legacy), "2026-08-15T12:00:00.123456+00:00");
        let exact: Timestamp = "2026-08-15T12:00:00Z".parse().unwrap();
        assert_eq!(format(exact), "2026-08-15T12:00:00+00:00");
        assert!(format(exact) < format(legacy));
    }
}
