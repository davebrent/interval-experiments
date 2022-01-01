use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use interval_experiments::{
    load_test_file, BaselineIntervalIndex, DefaultStatistics, Interval,
    IntervalIndex,
};

type BenchIndex = IntervalIndex<u64, DefaultStatistics<u64>>;
type BaselineIndex = BaselineIntervalIndex<u64, DefaultStatistics<u64>>;

fn interval_index_benchmarks(c: &mut Criterion) {
    let mut intervals: Vec<(u64, Interval)> =
        load_test_file("tests/256/intervals.txt")
            .iter()
            .map(|row| (row[0], Interval::new(row[1], row[2])))
            .collect();

    let queries: Vec<Interval> = load_test_file("tests/256/queries.txt")
        .iter()
        .map(|row| Interval::new(row[0], row[1]))
        .collect();

    intervals.sort_by_key(|(_, interval)| interval.start);

    // Subset of interesting queries from the main file (can blow up in size if
    // there is a bug in the index)
    let queries: Vec<_> = [2, 26, 49, 438, 471, 488]
        .iter()
        .map(|i| queries[*i])
        .collect();

    interval_index_writing_benchmarks(c, &intervals);
    interval_index_reading_benchmarks(c, &intervals, &queries);
}

fn interval_index_writing_benchmarks(
    c: &mut Criterion,
    intervals: &[(u64, Interval)],
) {
    let mut group = c.benchmark_group("writing");

    group.bench_function("implementation", |b| {
        b.iter(|| {
            let mut index = BenchIndex::new(4, 8);
            for (id, interval) in intervals {
                index.push(interval, *id);
            }
        });
    });

    group.bench_function("baseline", |b| {
        b.iter(|| {
            let mut index = BaselineIndex::new();
            for (id, interval) in intervals {
                index.push(interval, *id);
            }
        });
    });

    group.finish();
}

fn interval_index_reading_benchmarks(
    c: &mut Criterion,
    intervals: &[(u64, Interval)],
    queries: &[Interval],
) {
    let mut index = BenchIndex::new(4, 8);
    let mut baseline = BaselineIndex::new();

    for (id, interval) in intervals {
        index.push(interval, *id);
        baseline.push(interval, *id);
    }

    let mut group = c.benchmark_group("query");

    for (i, interval) in queries.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("baseline", i),
            interval,
            |b, interval| b.iter(|| baseline.query(interval)),
        );
        group.bench_with_input(
            BenchmarkId::new("implementation", i),
            interval,
            |b, interval| b.iter(|| index.query(interval)),
        );
    }

    group.finish();

    let mut group = c.benchmark_group("aggregate");

    for (i, interval) in queries.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("baseline", i),
            interval,
            |b, interval| b.iter(|| baseline.aggregate(interval)),
        );
        group.bench_with_input(
            BenchmarkId::new("implementation", i),
            interval,
            |b, interval| b.iter(|| index.aggregate(interval)),
        );
    }

    group.finish();
}

criterion_group!(benches, interval_index_benchmarks);
criterion_main!(benches);
