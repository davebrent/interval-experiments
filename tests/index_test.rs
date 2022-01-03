use interval_experiments::{
    load_test_file, BaselineIntervalIndex, DefaultStatistics, IntervalIndex,
};

type Index = IntervalIndex<u64, DefaultStatistics<u64>>;
type BaselineIndex = BaselineIntervalIndex<u64, DefaultStatistics<u64>>;

#[test]
#[ignore]
fn test_index_256() {
    let mut intervals = load_test_file("tests/256/intervals.txt");
    let queries = load_test_file("tests/256/queries.txt");

    intervals.sort_by_key(|row| row[1]);

    let mut actual = Index::new(8);
    let mut baseline = BaselineIndex::new();

    for row in intervals {
        let id = row[0];
        let start = row[1];
        let end = row[2];

        actual.push((start, end), id);
        baseline.push((start, end), id);
    }

    for (i, query) in queries.iter().enumerate() {
        let interval = (query[0], query[1]);

        let a = actual.aggregate(interval);
        let b = baseline.aggregate(interval);

        assert_eq!(a.count, b.count, "query {}", i);
        assert_eq!(a.min, b.min, "query {}", i);
        assert_eq!(a.max, b.max, "query {}", i);
        assert_eq!(a.total_duration, b.total_duration, "query {}", i);

        let a = actual.query(interval).flatten().collect::<Vec<_>>();
        let b = baseline.query(interval);

        assert_eq!(a.len(), b.len(), "query {}", i);
    }
}
