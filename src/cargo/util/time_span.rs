use std::time::Duration;

use crate::CargoResult;

/// Parses a time span string.
pub fn parse_time_span(span: &str) -> CargoResult<Duration> {
    maybe_parse_time_span(span).ok_or_else(|| {
        anyhow::format_err!(
            "expected a value of the form \
             \"N seconds/minutes/hours/days/weeks/months\", got: {span:?}"
        )
    })
}

/// Parses a time span string.
///
/// Returns None if the value is not valid. See [`parse_time_span`] if you
/// need a variant that generates an error message.
pub fn maybe_parse_time_span(span: &str) -> Option<Duration> {
    let Some(right_i) = span.find(|c: char| !c.is_ascii_digit()) else {
        return None;
    };
    let (left, mut right) = span.split_at(right_i);
    if right.starts_with(' ') {
        right = &right[1..];
    }
    let count: u64 = left.parse().ok()?;
    let factor = match right {
        "second" | "seconds" => 1,
        "minute" | "minutes" => 60,
        "hour" | "hours" => 60 * 60,
        "day" | "days" => 24 * 60 * 60,
        "week" | "weeks" => 7 * 24 * 60 * 60,
        "month" | "months" => 2_629_746, // average is 30.436875 days
        _ => return None,
    };
    Some(Duration::from_secs(factor * count))
}
