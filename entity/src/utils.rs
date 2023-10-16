use chrono::{DateTime, FixedOffset, Utc};
use std::fmt::Write;

/// Returns a string representing the time elapsed since the given time.
/// eg "2m1w2d3h4m5s ago"
pub fn time_ago(past: DateTime<FixedOffset>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(past);
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
        let _ = write!(&mut result, "{}w", years);
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
    if seconds > 0 || result.is_empty() {
        let _ = write!(&mut result, "{}s", seconds);
    }

    result.push_str(" ago");
    result
}
