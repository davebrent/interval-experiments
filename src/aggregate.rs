use std::marker::PhantomData;

use crate::interval::Interval;

pub struct MergeInfo {
    pub weight: f32,
}

pub trait Aggregate {
    type Value;
    fn initial() -> Self;
    fn aggregate(&mut self, interval: &Interval, value: &Self::Value);
    fn merge(&mut self, info: &MergeInfo, other: &Self);
}

pub struct DefaultStatistics<T> {
    pub min: u64,
    pub max: u64,
    pub total_duration: u64,
    pub count: usize,
    phantom: PhantomData<T>,
}

impl<T> Aggregate for DefaultStatistics<T> {
    type Value = T;

    fn initial() -> Self {
        Self {
            min: 0,
            max: 0,
            total_duration: 0,
            count: 0,
            phantom: PhantomData,
        }
    }

    fn aggregate(&mut self, interval: &Interval, _: &Self::Value) {
        let duration = interval.end - interval.start;
        self.min = duration.min(self.min);
        self.max = duration.max(self.max);
        self.total_duration += duration;
        self.count += 1;
    }

    fn merge(&mut self, info: &MergeInfo, other: &Self) {
        self.min = other.min.min(self.min);
        self.max = other.max.max(self.max);
        self.total_duration +=
            (other.total_duration as f64 * info.weight as f64) as u64;
        self.count += other.count;
    }
}
