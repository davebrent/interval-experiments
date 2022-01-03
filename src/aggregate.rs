use std::marker::PhantomData;

use crate::interval::Interval;

pub trait Aggregate {
    type Value;
    fn empty() -> Self;
    fn initial(interval: &Interval, value: &Self::Value) -> Self;
    fn aggregate(&mut self, other: &Self);
    fn weight(&mut self, _weight: f32) {}
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

    fn empty() -> Self {
        Self {
            min: 0,
            max: 0,
            total_duration: 0,
            count: 0,
            phantom: PhantomData,
        }
    }

    fn initial(interval: &Interval, _: &Self::Value) -> Self {
        let duration = interval.end - interval.start;
        Self {
            min: duration,
            max: duration,
            total_duration: duration,
            count: 1,
            phantom: PhantomData,
        }
    }

    fn aggregate(&mut self, other: &Self) {
        self.min = other.min.min(self.min);
        self.max = other.max.max(self.max);
        self.count += other.count;
        self.total_duration += other.total_duration;
    }

    fn weight(&mut self, weight: f32) {
        let duration = self.total_duration;
        self.total_duration = (duration as f64 * weight as f64) as u64;
    }
}
