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

    /// This time plus `minutes`, as `HH:MM` (for "paused until").
    pub fn hour_minute_after(self, minutes: u32) -> String {
        let total = (u32::from(self.hour) * 60 + u32::from(self.minute) + minutes) % (24 * 60);
        format!("{:02}:{:02}", total / 60, total % 60)
    }
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
    }
}
