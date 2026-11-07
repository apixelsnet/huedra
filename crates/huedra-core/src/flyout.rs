//! Where the tray's quick panel opens: beside the tray icon, on whichever
//! edge the taskbar sits, and always inside the monitor's work area.
//! Feature doc: `docs/features/07-tray-hotkeys.md`.

/// A rectangle in physical pixels, as Windows gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    /// Left edge.
    pub left: i32,
    /// Top edge.
    pub top: i32,
    /// Right edge (exclusive).
    pub right: i32,
    /// Bottom edge (exclusive).
    pub bottom: i32,
}

impl Rect {
    /// Width in pixels (never negative).
    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    /// Height in pixels (never negative).
    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    fn center_x(self) -> i32 {
        self.left + self.width() / 2
    }

    fn center_y(self) -> i32 {
        self.top + self.height() / 2
    }

    /// Reads `left top right bottom` as given on the command line.
    pub fn parse(args: &[String]) -> Option<Self> {
        let mut numbers = args.iter().map(|a| a.parse::<i32>().ok());
        match (
            numbers.next()?,
            numbers.next()?,
            numbers.next()?,
            numbers.next()?,
            numbers.next(),
        ) {
            (Some(left), Some(top), Some(right), Some(bottom), None) => Some(Self {
                left,
                top,
                right,
                bottom,
            }),
            _ => None,
        }
    }

    /// The four numbers for the command line.
    pub fn to_args(self) -> [String; 4] {
        [
            self.left.to_string(),
            self.top.to_string(),
            self.right.to_string(),
            self.bottom.to_string(),
        ]
    }
}

/// Keeps `start` so that `[start, start + size)` lies in `[low, high)`,
/// `margin` from both ends. Larger than the room: at `low`.
fn clamp(start: i32, size: i32, low: i32, high: i32, margin: i32) -> i32 {
    let min = low + margin;
    let max = high - margin - size;
    if max < min {
        return low;
    }
    start.clamp(min, max)
}

/// The top-left corner for a panel of `width` × `height` that belongs to
/// the tray icon at `icon`, inside the work area `work` (the monitor minus
/// the taskbar), `margin` pixels from its edges.
pub fn place(icon: Rect, work: Rect, width: i32, height: i32, margin: i32) -> (i32, i32) {
    let across = clamp(
        icon.center_x() - width / 2,
        width,
        work.left,
        work.right,
        margin,
    );
    let along = clamp(
        icon.center_y() - height / 2,
        height,
        work.top,
        work.bottom,
        margin,
    );
    if icon.top >= work.bottom {
        // Taskbar at the bottom, the usual place.
        (across, (work.bottom - margin - height).max(work.top))
    } else if icon.bottom <= work.top {
        (across, work.top + margin)
    } else if icon.right <= work.left {
        (work.left + margin, along)
    } else if icon.left >= work.right {
        ((work.right - margin - width).max(work.left), along)
    } else {
        // The icon is inside the work area: in the overflow area (^) or on
        // a taskbar that hides itself. Open above it when there is room.
        let above = icon.top - margin - height;
        let y = if above >= work.top + margin {
            above
        } else {
            clamp(icon.bottom + margin, height, work.top, work.bottom, margin)
        };
        (across, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1032,
    };

    fn icon(left: i32, top: i32) -> Rect {
        Rect {
            left,
            top,
            right: left + 24,
            bottom: top + 24,
        }
    }

    #[test]
    fn opens_on_the_taskbar_edge() {
        assert_eq!(
            place(icon(1700, 1044), WORK, 540, 510, 12),
            (1368, 510),
            "bottom: above the taskbar"
        );
        let top = Rect {
            top: 48,
            bottom: 1080,
            ..WORK
        };
        assert_eq!(
            place(icon(1700, 12), top, 540, 510, 12),
            (1368, 60),
            "top: below the taskbar"
        );
        let left = Rect { left: 48, ..WORK };
        assert_eq!(
            place(icon(12, 900), left, 540, 510, 12),
            (60, 510),
            "left: beside the taskbar, kept on screen"
        );
        let right = Rect {
            right: 1872,
            ..WORK
        };
        assert_eq!(
            place(icon(1884, 300), right, 540, 510, 12),
            (1320, 57),
            "right"
        );
    }

    #[test]
    fn stays_in_the_work_area() {
        assert_eq!(
            place(icon(1890, 1044), WORK, 540, 510, 12).0,
            1368,
            "icon at the right edge"
        );
        assert_eq!(
            place(icon(0, 1044), WORK, 540, 510, 12).0,
            12,
            "icon at the left edge"
        );
        let small = Rect {
            right: 400,
            bottom: 300,
            ..WORK
        };
        assert_eq!(
            place(icon(380, 310), small, 540, 510, 12),
            (0, 0),
            "larger than the screen: from the top left"
        );
    }

    #[test]
    fn overflow_area_opens_above_the_icon() {
        assert_eq!(
            place(icon(1600, 900), WORK, 540, 510, 12),
            (1342, 378),
            "above"
        );
        assert_eq!(
            place(icon(1600, 100), WORK, 540, 510, 12),
            (1342, 136),
            "below when there is no room above"
        );
    }

    #[test]
    fn parses_the_command_line() {
        let args = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let rect = Rect {
            left: -10,
            top: 20,
            right: 30,
            bottom: 40,
        };
        assert_eq!(
            Rect::parse(&args("-10 20 30 40")),
            Some(rect),
            "four numbers"
        );
        assert_eq!(Rect::parse(&rect.to_args()), Some(rect), "round trip");
        assert_eq!(Rect::parse(&args("1 2 3")), None, "too few");
        assert_eq!(Rect::parse(&args("1 2 3 4 5")), None, "too many");
        assert_eq!(Rect::parse(&args("1 2 x 4")), None, "not a number");
    }
}
