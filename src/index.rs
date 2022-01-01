use std::marker::PhantomData;
use std::ops::Range;

use crate::aggregate::{Aggregate, MergeInfo};
use crate::interval::Interval;

pub trait QueryVisitor<V, A> {
    fn visit_fast_lane(&mut self, lane: &FastLane<V, A>, index: usize);
    fn visit_slow_lane(&mut self, lane: &SlowLane<V>, index: usize);
}

pub struct IntervalIndex<V, A> {
    base_size: usize,
    fast_lanes: Vec<FastLane<V, A>>,
    slow_lane: SlowLane<V>,
}

#[derive(Clone, Debug)]
pub struct FastLane<V, A> {
    interval: usize,
    intervals: Vec<Interval>,
    aggregations: Vec<A>,
    phantom: PhantomData<V>,
}

pub struct SlowLane<V> {
    intervals: Vec<Interval>,
    values: Vec<V>,
}

struct AggregateVisitor<V, A> {
    output: A,
    phantom: PhantomData<V>,
}

struct RangeVisitor<'a, V> {
    count: usize,
    slow_lane: &'a SlowLane<V>,
    output: Vec<Range<usize>>,
}

impl<V, A> FastLane<V, A>
where
    A: Aggregate<Value = V>,
{
    pub fn new(interval: usize) -> Self {
        FastLane {
            interval,
            intervals: vec![],
            aggregations: vec![],
            phantom: PhantomData,
        }
    }

    pub fn push(&mut self, index: usize, interval: Interval, value: &V) {
        if self.intervals.is_empty() || index % self.interval == 0 {
            let mut aggregate = A::initial();
            aggregate.aggregate(&interval, value);

            self.intervals.push(interval);
            self.aggregations.push(aggregate);
        } else {
            let index = index / self.interval;
            let other = &mut self.intervals[index];
            other.end = other.end.max(interval.end);

            self.aggregations[index].aggregate(&interval, value);
        }
    }
}

impl<V> SlowLane<V> {
    fn len(&self) -> usize {
        self.intervals.len()
    }

    fn push(&mut self, interval: Interval, value: V) {
        self.intervals.push(interval);
        self.values.push(value);
    }
}

impl<V, A> IntervalIndex<V, A>
where
    A: Aggregate<Value = V>,
{
    pub fn new(max_lanes: usize, base_size: usize) -> Self {
        let slow_lane = SlowLane {
            intervals: vec![],
            values: vec![],
        };

        let mut fast_lanes: Vec<_> = (0..max_lanes)
            .map(|i| FastLane::new(base_size.pow(i as u32 + 1)))
            .collect();

        fast_lanes.reverse();

        Self {
            base_size,
            fast_lanes,
            slow_lane,
        }
    }

    pub fn push<I>(&mut self, interval: I, value: V)
    where
        I: Into<Interval>,
    {
        let interval = interval.into();
        let index = self.slow_lane.len();

        for lane in &mut self.fast_lanes {
            lane.push(index, interval, &value);
        }

        self.slow_lane.push(interval, value);
    }

    fn first_fastlane_overlap(&self, window: Interval) -> usize {
        let mut offset = 0;

        for lane in &self.fast_lanes {
            for interval in &lane.intervals[offset..] {
                if interval.overlaps(window) {
                    break;
                }
                offset += 1;
            }
            offset = offset.checked_sub(1).unwrap_or(0) * self.base_size;
        }

        offset
    }

    fn first_overlap(&self, window: Interval) -> usize {
        let index = self.first_fastlane_overlap(window);
        let slice = &self.slow_lane.intervals[index..];

        for (i, interval) in slice.iter().enumerate() {
            if interval.overlaps(window) {
                return i + index;
            }
        }

        index
    }

    pub fn aggregate<I>(&self, window: I) -> A
    where
        I: Into<Interval>,
    {
        let mut visitor = AggregateVisitor {
            output: A::initial(),
            phantom: PhantomData,
        };
        self.query_with(window, &mut visitor);
        visitor.output
    }

    pub fn query<I>(&self, window: I) -> Vec<&V>
    where
        I: Into<Interval>,
    {
        let mut visitor = RangeVisitor {
            slow_lane: &self.slow_lane,
            output: vec![],
            count: 0,
        };

        self.query_with(window, &mut visitor);

        let mut output = Vec::with_capacity(visitor.count);
        for range in visitor.output {
            output.extend(&self.slow_lane.values[range]);
        }
        output
    }

    pub fn query_with<I, Q>(&self, window: I, visitor: &mut Q)
    where
        I: Into<Interval>,
        Q: QueryVisitor<V, A>,
    {
        let window = window.into();
        let length = self.slow_lane.intervals.len();

        let mut index = self.first_overlap(window);

        'search: while index < length {
            // Try and advance the index along the highest/coarsest fast lane.
            // This can only be done if the fast lane interval is completely
            // contained by the query window.
            for lane in &self.fast_lanes {
                let lane_index = index / lane.interval;
                match lane.intervals.get(lane_index) {
                    Some(interval) if window.contains(*interval) => {}
                    _ => continue,
                };
                visitor.visit_fast_lane(lane, index);
                index += lane.interval;
                continue 'search;
            }

            // Otherwise advance to the next multiple of `base_size` in the slow
            // lane, before trying again with the fast lane.
            let iterations = self.base_size - (index % self.base_size);
            let start = index.min(length - 1);
            let end = (index + iterations).min(length);
            let intervals = &self.slow_lane.intervals[start..end];

            for (i, interval) in intervals.iter().enumerate() {
                if interval.start > window.end {
                    break 'search;
                }
                if interval.overlaps(window) {
                    visitor.visit_slow_lane(&self.slow_lane, index + i);
                }
            }

            index = end;
        }
    }
}

impl<V, A> QueryVisitor<V, A> for AggregateVisitor<V, A>
where
    A: Aggregate<Value = V>,
{
    fn visit_fast_lane(&mut self, lane: &FastLane<V, A>, index: usize) {
        let lane_index = index / lane.interval;
        let info = MergeInfo { weight: 1.0 };
        self.output.merge(&info, &lane.aggregations[lane_index]);
    }

    fn visit_slow_lane(&mut self, lane: &SlowLane<V>, index: usize) {
        let info = MergeInfo { weight: 1.0 };
        let mut aggregate = A::initial();
        aggregate.aggregate(&lane.intervals[index], &lane.values[index]);
        self.output.merge(&info, &aggregate);
    }
}

impl<'a, V, A> QueryVisitor<V, A> for RangeVisitor<'a, V>
where
    A: Aggregate<Value = V>,
{
    fn visit_fast_lane(&mut self, lane: &FastLane<V, A>, index: usize) {
        let end = (index + lane.interval).min(self.slow_lane.len());
        self.count += end - index;
        match self.output.last_mut() {
            Some(prev) if prev.end == index => prev.end = end,
            _ => self.output.push(index..end),
        };
    }

    fn visit_slow_lane(&mut self, _: &SlowLane<V>, index: usize) {
        let end = index + 1;
        self.count += 1;
        match self.output.last_mut() {
            Some(prev) if prev.end == index => prev.end = end,
            _ => self.output.push(index..end),
        };
    }
}
