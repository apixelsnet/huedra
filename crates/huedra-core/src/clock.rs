//! Wall-clock time as read from Windows, formatted for people.

use std::fmt;

/// A local date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocalTime {
    /// Year, for example 2026.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u16,
    /// Day of the month.
    pub day: u16,
    /// Hour, 0 to 23.
    pub hour: u16,
    /// Minute.
    pub minute: u16,
    /// Second.
    pub second: u16,
}

impl LocalTime {
    /// `HH:MM`, as in the history list.
    pub fn hour_minute(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// `YYYY-MM-DD`, the day the history list groups by.
    pub fn date(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// This time plus `minutes`, as `HH:MM` (for "paused until").
    pub fn hour_minute_after(self, minutes: u32) -> String {
        let total = (u32::from(self.hour) * 60 + u32::from(self.minute) + minutes) % (24 * 60);
        format!("{:02}:{:02}", total / 60, total % 60)
    }
}

/// The heading of a day in the history list: "Today", "Yesterday" or, for
/// example, "November 5". Both days are `YYYY-MM-DD`; a day that cannot be
/// read is shown as it is.
pub fn day_label(day: &str, today: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let (Some(d), Some(t)) = (day_number(day), day_number(today)) else {
        return day.to_owned();
    };
    if d == t {
        return "Today".to_owned();
    }
    if d + 1 == t {
        return "Yesterday".to_owned();
    }
    let mut parts = day.split('-').skip(1).map(str::parse::<usize>);
    match (parts.next(), parts.next()) {
        (Some(Ok(month)), Some(Ok(dom))) if (1..=12).contains(&month) => {
            format!("{} {dom}", MONTHS[month - 1])
        }
        _ => day.to_owned(),
    }
}

/// The day before a `YYYY-MM-DD` day, or `None` when it is not one.
pub fn day_before(day: &str) -> Option<String> {
    // Civil from days (Howard Hinnant's algorithm).
    let z = day_number(day)? - 1 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` day (the civil-from-days
/// algorithm run backwards), or `None` when it is not one.
fn day_number(day: &str) -> Option<i64> {
    let mut parts = day.split('-').map(str::parse::<i64>);
    let (Some(Ok(y)), Some(Ok(m)), Some(Ok(d)), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

impl fmt::Display for LocalTime {
    /// `YYYY-MM-DD HH:MM:SS`, as in the log.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let t = LocalTime {
            year: 2026,
            month: 10,
            day: 31,
            hour: 23,
            minute: 30,
            second: 5,
        };
        assert_eq!(t.to_string(), "2026-10-31 23:30:05", "log stamp");
        assert_eq!(t.hour_minute(), "23:30", "history");
        assert_eq!(t.hour_minute_after(60), "00:30", "wraps at midnight");
        assert_eq!(t.date(), "2026-10-31", "day");
    }

    #[test]
    fn day_labels() {
        assert_eq!(day_label("2026-11-07", "2026-11-07"), "Today", "same day");
        assert_eq!(
            day_label("2026-11-06", "2026-11-07"),
            "Yesterday",
            "day before"
        );
        assert_eq!(
            day_label("2026-10-31", "2026-11-01"),
            "Yesterday",
            "across months"
        );
        assert_eq!(
            day_label("2025-12-31", "2026-01-01"),
            "Yesterday",
            "across years"
        );
        assert_eq!(
            day_label("2028-02-29", "2028-03-01"),
            "Yesterday",
            "leap day"
        );
        assert_eq!(day_label("2026-11-05", "2026-11-07"), "November 5", "older");
        assert_eq!(day_label("", "2026-11-07"), "", "unknown day");
        assert_eq!(
            day_label("2026-13-01", "2026-11-07"),
            "2026-13-01",
            "not a day"
        );
    }

    #[test]
    fn days_before() {
        assert_eq!(
            day_before("2026-11-07").as_deref(),
            Some("2026-11-06"),
            "same month"
        );
        assert_eq!(
            day_before("2026-03-01").as_deref(),
            Some("2026-02-28"),
            "after February"
        );
        assert_eq!(
            day_before("2028-03-01").as_deref(),
            Some("2028-02-29"),
            "leap year"
        );
        assert_eq!(
            day_before("2026-01-01").as_deref(),
            Some("2025-12-31"),
            "new year"
        );
        assert_eq!(day_before("soon"), None, "not a day");
    }
}
