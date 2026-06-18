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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_spans() {
        let d = |x| Some(Duration::from_secs(x));
        assert_eq!(maybe_parse_time_span("0 seconds"), d(0));
        assert_eq!(maybe_parse_time_span("1second"), d(1));
        assert_eq!(maybe_parse_time_span("23 seconds"), d(23));
        assert_eq!(maybe_parse_time_span("5 minutes"), d(60 * 5));
        assert_eq!(maybe_parse_time_span("2 hours"), d(60 * 60 * 2));
        assert_eq!(maybe_parse_time_span("1 day"), d(60 * 60 * 24));
        assert_eq!(maybe_parse_time_span("2 weeks"), d(60 * 60 * 24 * 14));
        assert_eq!(maybe_parse_time_span("6 months"), d(2_629_746 * 6));
    }

    #[test]
    fn time_span_errors() {
        assert_eq!(maybe_parse_time_span(""), None);
        assert_eq!(maybe_parse_time_span("1"), None);
        assert_eq!(maybe_parse_time_span("second"), None);
        assert_eq!(maybe_parse_time_span("+2 seconds"), None);
        assert_eq!(maybe_parse_time_span("day"), None);
        assert_eq!(maybe_parse_time_span("-1 days"), None);
        assert_eq!(maybe_parse_time_span("1.5 days"), None);
        assert_eq!(maybe_parse_time_span("1 dayz"), None);
        assert_eq!(maybe_parse_time_span("always"), None);
        assert_eq!(maybe_parse_time_span("never"), None);
        assert_eq!(maybe_parse_time_span("1 day "), None);
        assert_eq!(maybe_parse_time_span(" 1 day"), None);
        assert_eq!(maybe_parse_time_span("1  second"), None);
    }
}
