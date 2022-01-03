use std::fs::read_to_string;
use std::marker::PhantomData;
use std::path::Path;

use crate::aggregate::Aggregate;
use crate::interval::Interval;

pub struct BaselineIntervalIndex<V, A> {
    intervals: Vec<Interval>,
    values: Vec<V>,
    phantom: PhantomData<A>,
}

pub fn load_test_file<P>(path: P) -> Vec<Vec<u64>>
where
    P: AsRef<Path>,
{
    read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| {
            line.split(" ").map(|p| p.parse::<u64>().unwrap()).collect()
        })
        .collect()
}

impl<V, A> BaselineIntervalIndex<V, A>
where
    A: Aggregate<Value = V>,
{
    pub fn new() -> Self {
        Self {
            intervals: vec![],
            values: vec![],
            phantom: PhantomData,
        }
    }

    pub fn push<I>(&mut self, interval: I, value: V)
    where
        I: Into<Interval>,
    {
        self.intervals.push(interval.into());
        self.values.push(value);
    }

    pub fn query<I>(&self, window: I) -> Vec<&V>
    where
        I: Into<Interval>,
    {
        let window = window.into();
        let mut output = vec![];

        for (i, interval) in self.intervals.iter().enumerate() {
            if interval.overlaps(window) {
                output.push(&self.values[i]);
            }
            if interval.start > window.end {
                break;
            }
        }

        output
    }

    pub fn aggregate<I>(&self, window: I) -> A
    where
        I: Into<Interval>,
    {
        let window = window.into();
        let mut out = A::empty();

        for (i, interval) in self.intervals.iter().enumerate() {
            if interval.start > window.end {
                break;
            }
            if interval.overlaps(window) {
                let value = &self.values[i];
                out.aggregate(&A::initial(interval, value));
            }
        }

        out
    }
}
