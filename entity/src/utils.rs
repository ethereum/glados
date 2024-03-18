use chrono::{DateTime, Utc};
use std::fmt::Write;

/// Returns a string representing the time elapsed since the given time.
/// eg "2m1w2d3h4m5s ago"
pub fn time_ago(past: DateTime<Utc>, present: DateTime<Utc>) -> String {
    let duration = present.signed_duration_since(past);
    if duration.num_seconds() < 0 {
        return "Time is in the future".to_string();
    }

    let years = duration.num_days() / 365;
    let days = duration.num_days() - (years * 365);
    let hours = duration.num_hours() - (duration.num_days() * 24);
    let minutes = duration.num_minutes() - (duration.num_hours() * 60);
    let seconds = duration.num_seconds() - (duration.num_minutes() * 60);

    let mut result = String::new();

    if years > 0 {
        let _ = write!(&mut result, "{}y", years);
    }
    if days > 0 {
        let _ = write!(&mut result, "{}d", days);
    }
    if hours > 0 {
        let _ = write!(&mut result, "{}h", hours);
    }
    if minutes > 0 {
        let _ = write!(&mut result, "{}m", minutes);
    }
    // Don't show seconds after a day
    if (seconds > 0 || result.is_empty()) && (days == 0) {
        let _ = write!(&mut result, "{}s", seconds);
    }

    result.push_str(" ago");
    result
}

#[cfg(test)]
mod test {
    use super::time_ago;
    use chrono::{TimeDelta, Utc};
    use rstest::rstest;

    #[rstest]
    #[case(0, "0s ago")]
    #[case(10, "10s ago")]
    #[case(90, "1m30s ago")]
    #[case(3600, "1h ago")]
    #[case(356523, "4d3h2m ago")]
    #[case(31798861, "1y3d1h1m ago")]
    fn test_time_ago(#[case] seconds_ago: i64, #[case] expected_result: &str) {
        let seconds_ago_delta =
            TimeDelta::try_seconds(seconds_ago).expect("Couldn't calculate seconds delta.");
        assert_eq!(
            time_ago(Utc::now() - seconds_ago_delta, Utc::now()),
            expected_result
        );
    }
}
