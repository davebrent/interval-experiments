use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Range;

use crate::aggregate::Aggregate;
use crate::interval::Interval;

pub trait QueryVisitor<V, A> {
    fn visit_fast_lane(&mut self, lane: &FastLane<V, A>, index: usize);
    fn visit_slow_lane(&mut self, lane: &SlowLane<V>, index: usize);
}

pub struct IntervalIndex<V, A> {
    pub order: usize,
    pub max_top_level: usize,
    pub fast_lanes: Vec<FastLane<V, A>>,
    pub slow_lane: SlowLane<V>,
}

#[derive(Clone, Debug)]
pub struct FastLane<V, A> {
    pub interval: usize,
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
    pub fn new(interval: usize, capacity: usize) -> FastLane<V, A> {
        FastLane {
            interval,
            intervals: Vec::with_capacity(capacity),
            aggregations: Vec::with_capacity(capacity),
            phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    fn push(&mut self, interval: Interval, aggregate: A) {
        self.intervals.push(interval);
        self.aggregations.push(aggregate);
    }

    fn update(&mut self, index: usize, interval: Interval, aggregate: A) {
        let other = &mut self.intervals[index];
        other.end = other.end.max(interval.end);
        self.aggregations[index].aggregate(&aggregate);
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
    pub fn new(order: usize) -> Self {
        let max_top_level = 4096 / size_of::<Interval>();

        let slow_lane = SlowLane {
            // If we expect N elements in the initial fast lane, then we are
            // expecting N * order elements in the slow lane
            intervals: Vec::with_capacity(order * max_top_level),
            values: Vec::with_capacity(order * max_top_level),
        };

        let fast_lanes = vec![FastLane::new(order, max_top_level)];

        Self {
            order,
            max_top_level,
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
            let aggregate = A::initial(&interval, &value);
            if index % lane.interval == 0 {
                lane.push(interval, aggregate);
            } else {
                lane.update(index / lane.interval, interval, aggregate);
            }
        }

        self.slow_lane.push(interval, value);

        if self.fast_lanes[0].len() == self.max_top_level {
            self.rebuild_top_level();
        }
    }

    fn rebuild_top_level(&mut self) {
        let top = &self.fast_lanes[0];

        let interval = self.order.pow(self.fast_lanes.len() as u32 + 1);
        let mut fast_lane = FastLane::new(interval, self.max_top_level);

        let items = top.intervals.iter().zip(top.aggregations.iter());
        for (i, (interval, aggregate)) in items.enumerate() {
            let mut copy = A::empty();
            copy.aggregate(&aggregate);

            if i % self.order == 0 {
                fast_lane.push(*interval, copy);
            } else {
                fast_lane.update(i / self.order, *interval, copy);
            }
        }

        self.fast_lanes.insert(0, fast_lane);
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
            offset *= self.order;
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
            output: A::empty(),
            phantom: PhantomData,
        };
        self.query_with(window, &mut visitor);
        visitor.output
    }

    pub fn query<I>(&self, window: I) -> impl Iterator<Item = &[V]>
    where
        I: Into<Interval>,
    {
        let mut visitor = RangeVisitor {
            slow_lane: &self.slow_lane,
            output: vec![],
            count: 0,
        };

        self.query_with(window, &mut visitor);

        visitor
            .output
            .into_iter()
            .map(move |range| &self.slow_lane.values[range])
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
                    Some(interval) if window.contains(*interval) => {
                        visitor.visit_fast_lane(lane, index);
                        index += lane.interval;
                        continue 'search;
                    }
                    _ => continue,
                };
            }

            // Otherwise advance to the next multiple of `order` in the slow
            // lane, before trying again with the fast lane.
            let iterations = self.order - (index % self.order);
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
        self.output.aggregate(&lane.aggregations[lane_index]);
    }

    fn visit_slow_lane(&mut self, lane: &SlowLane<V>, index: usize) {
        let interval = &lane.intervals[index];
        let value = &lane.values[index];
        self.output.aggregate(&A::initial(interval, value));
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
