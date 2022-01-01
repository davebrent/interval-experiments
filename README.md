# Interval experiments

An experiment with implementing an [augmented interval tree][interval_tree],
with a breadth first layout to accelerate queries on collections of intervals.

  [interval_tree]: https://en.wikipedia.org/wiki/Interval_tree#Augmented_tree
  [trishume]: https://twitter.com/trishume
  [iforest]: https://thume.ca/2021/03/14/iforests/
  [cssl]: https://www2.informatik.hu-berlin.de/~sprengsz/papers/cssl.pdf

Semi inspired by [@trishume][trishume]'s [IForestIndex][iforest] data structure
and [Cache-Sensitive Skip List: Effiecient Range Queries on modern CPUs][cssl].
Its similar to the `IForestIndex`, but with a breadth first layout and an array
for each level.

Below is a reminder to myself what was going on.

## The data

The input is an immutable collection of key/value pairs, where the key is an
interval with a `start` and `end` time. The collection is pre-sorted by the
`start` time of each interval.

```
+-A-------------------------------+
     +-B-----+      +-C----------------+

Result = [(A, ...), (B, ...), (C, ...)]
```

## The goal

* Return all values that occur within (and overlap with) a query window.
* Support custom statistics on values that occur within a (and overlap with) a
  query interval.
* Achieve the above while being, simple, fast, low memory, cache efficient etc.

```
                        [ Query Window    ]
                        |                 |
+-A---------------------|---------+       |
     +-B-----+      +-C-|--------------+  |
                        |                 |
                        [                 ]

Result = [(A, ...), (C, ...)]
```

## The idea

Construct a segment tree, with a breadth first layout, with a separate array
for each level.

Or in terms of a balanced skip list, the key value pairs live in flat arrays and
there is a hierarchy of "fast lanes" contain every "nth" interval of the lane
before it.

```
----------+---------------------------------------+----------------------------
lane 0    | 0                                     | 1                    
----------+-------------------+-------------------+-------------------+--------
lane 1    | 0                 | 1                 | 2                 | 3
----------+---------+---------+---------+---------+---------+---------+--------
lane 2    | 0       | 1       | 2       | 3       | 4       | 5       | 6
----------+----+----+----+----+----+----+----+----+----+----+----+----+----+---
intervals | 0  | 1  | 2  | 3  | 4  | 5  | 6  | 7  | 8  | 9  | 10 | 11 | 12 |
values    | .. | .. | .. | .. | .. | .. | .. | .. | .. | .. | .. | .. | .. | ..
----------+----+----+----+----+----+----+----+----+----+----+----+----+----+---

Lanes = 3
Order = 2
```

## Appending

When a key/value pair is appended, the "parent" element in each lane is updated
with the new intervals `end` time. In psudo rust code the update looks something
like below:

```rust
let index = intervals.len();
intervals.push(interval);

for lane in lanes {
  if index % lane.interval == 0 {
    // Add the interval
    lane.intervals.push(interval);
  } else {
    // Update the existing interval
    let lane_index = index / lane.interval;
    lane[lane_index].end = lane[lane_index].max(interval.end);
  }
}
```

This update doesn't have to just be with the new intervals `end` time, but could
be any operation that makes sense aggregated over a range of values (the `sum`
of some value being the obvious/classic one).

In the code this is the `Aggregated` type `A`.

As this has to be calculated when an element is appended to the data structure,
the whole data structure gets parameterized over this generic type `A`.

## Searching

Searching for intervals within some window is achieved by, searching through the
first fast lane until an overlap with the window is found.

When an overlap is found the search moves down to the next lane at the
corresponding position, repeating until there are no more fast lanes.

This process returns a rough index into the intervals array where a linear scan
can then be done to find the very first interval that overlaps the window.

Once the first interval is found the search process then trys to search back up
the tree finding the largest possible element in a fast lane that is completely
contained within the window, that skips the most elements. Otherwise it
continues scanning through the intervals array.

The process terminates when an interval is encountered that starts at the
`end` of the window (`interval.start > window.end`) or the end of the array is
reached.

```
----------+---------------------------------------+----------------------------
lane 0    | 0                                     |
----------+-------------------+-------------------+-------------------+--------
lane 1    | 1                 | 2                 | 8                 |
----------+---------+---------+---------+---------+---------+---------+--------
lane 2    |         |         | 3       | 4       |         |         | ..
----------+----+----+----+----+----+----+----+----+----+----+----+----+----+---
intervals |    |    |    |    |    | 5  | 6  | 7  |    |    |    |    | 9  |
----------+----+----+----+----+----+----+--^-+----+----+----+----+----+--^-+---
                                           |                             |
                                           +-----------------------------+
                                                    Query Window

Result =
                                        +----+----+-------------------+----+
                                        | 6  | 7  | 8                 | 9  |
                                        +----+----+-------------------+----+

(The numbers are the order in which the elements are visited)
```

Important to note, its possible for there to be an interval at element 7 thats
not in the query window, see the scenario below.

```
                                        +----+    +-------------------+----+
                                        | 6  |    | 8                 | 9  |
                                        +--^-+    +-------------------+--^-+
                                           |                             |
                          +-6--------------|----------------+            |
                               +-7-----+   |  +-8----------------+    +-9|---+
                                           |                             |
                                           +-----------------------------+
                                                    Query Window
```

Using this process its then relatively simple to either merge multiple
`Aggregated` values, that may represent many intervals, into a single result OR
return a series of contiguous ranges of values that fall in the query window.
