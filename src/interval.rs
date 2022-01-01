pub type Timestamp = u64;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct Interval {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl From<(Timestamp, Timestamp)> for Interval {
    fn from(i: (Timestamp, Timestamp)) -> Interval {
        Interval {
            start: i.0,
            end: i.1,
        }
    }
}

impl From<&Interval> for Interval {
    fn from(interval: &Interval) -> Interval {
        *interval
    }
}

impl Interval {
    pub fn new(start: Timestamp, end: Timestamp) -> Self {
        Interval { start, end }
    }

    #[inline]
    pub fn overlaps(&self, other: Self) -> bool {
        self.start <= other.end && self.end >= other.start
    }

    #[inline]
    pub fn contains(&self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_overlaps_contains() {
        let a = Interval::new(0, 10);
        let b = Interval::new(5, 6);
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));
    }

    #[test]
    fn test_interval_overlaps_lhs() {
        let a = Interval::new(5, 10);
        let b = Interval::new(3, 6);
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));
    }

    #[test]
    fn test_interval_overlaps_rhs() {
        let a = Interval::new(5, 10);
        let b = Interval::new(8, 12);
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));
    }

    #[test]
    fn test_interval_overlaps_none_lhs() {
        let a = Interval::new(5, 6);
        let b = Interval::new(0, 1);
        assert!(!(a.overlaps(b)));
        assert!(!(b.overlaps(a)));
    }

    #[test]
    fn test_interval_overlaps_none_rhs() {
        let a = Interval::new(0, 4);
        let b = Interval::new(5, 11);
        assert!(!(a.overlaps(b)));
        assert!(!(b.overlaps(a)));
    }
}
