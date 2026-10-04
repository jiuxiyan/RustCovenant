use toodee::{TooDee, TooDeeOps};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

#[derive(Debug)]
struct Datum {
    id: u32,
    drops: Arc<AtomicUsize>,
}

impl Datum {
    fn new(id: u32, drops: &Arc<AtomicUsize>) -> Self {
        Self {
            id,
            drops: Arc::clone(drops),
        }
    }
}

impl Drop for Datum {
    fn drop(&mut self) {
        self.drops.fetch_add(1, AtomicOrdering::SeqCst);
    }
}

// ExactSizeIterator that lies about its length (declares full row length but yields fewer items).
struct WrongLengthIter {
    declared: usize,
    items: Vec<Datum>,
}

impl Iterator for WrongLengthIter {
    type Item = Datum;
    fn next(&mut self) -> Option<Datum> {
        // Pop is deterministic for our purposes; ordering is not relied upon on the failing path.
        self.items.pop()
    }
}

impl ExactSizeIterator for WrongLengthIter {
    fn len(&self) -> usize {
        self.declared
    }
}

// ExactSizeIterator that panics after yielding one element.
struct PanicAfterOne {
    first: Option<Datum>,
}

impl Iterator for PanicAfterOne {
    type Item = Datum;
    fn next(&mut self) -> Option<Datum> {
        if let Some(v) = self.first.take() {
            Some(v)
        } else {
            panic!("controlled iterator failure")
        }
    }
}

impl ExactSizeIterator for PanicAfterOne {
    fn len(&self) -> usize {
        3
    }
}

fn row_ids(g: &TooDee<Datum>, r: usize) -> Vec<u32> {
    g[r].iter().map(|d| d.id).collect()
}

fn main() {
    // Scenario 1: insert_row with a valid exact-size iterator preserves order and drops exactly once.
    {
        let drops = Arc::new(AtomicUsize::new(0));

        let initial: Vec<Datum> = (0u32..6)
            .map(|i| Datum::new(i, &drops))
            .collect();
        let mut grid = TooDee::from_vec(3, 2, initial);

        let inserted: Vec<Datum> = (100u32..103)
            .map(|i| Datum::new(i, &drops))
            .collect();
        grid.insert_row(1, inserted);

        assert_eq!(grid.num_cols(), 3);
        assert_eq!(grid.num_rows(), 3);
        assert_eq!(row_ids(&grid, 0), vec![0, 1, 2]);
        assert_eq!(row_ids(&grid, 1), vec![100, 101, 102]);
        assert_eq!(row_ids(&grid, 2), vec![3, 4, 5]);

        drop(grid);
        assert_eq!(
            drops.load(AtomicOrdering::SeqCst),
            9,
            "all moved-in values must be dropped exactly once on normal completion"
        );
    }

    // Scenario 2: malformed short iterator must be rejected (panic) and must not cause duplicate drops.
    {
        let drops = Arc::new(AtomicUsize::new(0));

        let initial: Vec<Datum> = (0u32..3)
            .map(|i| Datum::new(i, &drops))
            .collect();
        let mut grid = TooDee::from_vec(3, 1, initial);

        // Declares 3 but yields only 2.
        let lying = WrongLengthIter {
            declared: 3,
            items: vec![Datum::new(10, &drops), Datum::new(11, &drops)],
        };
        let total_created = 3usize + 2usize;

        let res = catch_unwind(AssertUnwindSafe(|| {
            grid.insert_row(1, lying);
        }));
        assert!(res.is_err(), "short iterator must be rejected");

        drop(grid);
        let d = drops.load(AtomicOrdering::SeqCst);
        assert!(
            d <= total_created,
            "must not duplicate-destroy values on short-iterator failure (drops={d}, created={total_created})"
        );
    }

    // Scenario 3: iterator panic during insert_row must not cause duplicate drops.
    {
        let drops = Arc::new(AtomicUsize::new(0));

        let initial: Vec<Datum> = (0u32..3)
            .map(|i| Datum::new(i, &drops))
            .collect();
        let mut grid = TooDee::from_vec(3, 1, initial);

        let it = PanicAfterOne {
            first: Some(Datum::new(99, &drops)),
        };
        let total_created = 3usize + 1usize;

        let res = catch_unwind(AssertUnwindSafe(|| {
            grid.insert_row(1, it);
        }));
        assert!(res.is_err(), "iterator panics must propagate as a panic");

        drop(grid);
        let d = drops.load(AtomicOrdering::SeqCst);
        assert!(
            d <= total_created,
            "must not duplicate-destroy values on iterator panic (drops={d}, created={total_created})"
        );
    }

    // Scenario 4: remove_col drains exactly that column; dropping the drain restores array shape.
    {
        let drops = Arc::new(AtomicUsize::new(0));

        let initial: Vec<Datum> = (0u32..6)
            .map(|i| Datum::new(i, &drops))
            .collect();
        let mut grid = TooDee::from_vec(3, 2, initial);

        let drained: Vec<Datum> = {
            let drain = grid.remove_col(1);
            drain.collect()
        };

        assert_eq!(grid.num_cols(), 2);
        assert_eq!(grid.num_rows(), 2);
        assert_eq!(row_ids(&grid, 0), vec![0, 2]);
        assert_eq!(row_ids(&grid, 1), vec![3, 5]);
        assert_eq!(drained.iter().map(|d| d.id).collect::<Vec<u32>>(), vec![1, 4]);

        drop(drained);
        drop(grid);
        assert_eq!(
            drops.load(AtomicOrdering::SeqCst),
            6,
            "remove_col should move out exactly one element per row, all dropped exactly once"
        );
    }

    // Scenario 5: dropping a partially-consumed DrainCol drops remaining drained items and updates dimensions.
    {
        let drops = Arc::new(AtomicUsize::new(0));

        let initial: Vec<Datum> = (0u32..6)
            .map(|i| Datum::new(i, &drops))
            .collect();
        let mut grid = TooDee::from_vec(2, 3, initial);

        let mut drain = grid.remove_col(0);
        let first = drain.next().expect("first drained element exists");
        assert_eq!(first.id, 0);
        drop(first);

        drop(drain);
        assert_eq!(grid.num_cols(), 1);
        assert_eq!(grid.num_rows(), 3);
        assert_eq!(grid[(0, 0)].id, 1);
        assert_eq!(grid[(0, 1)].id, 3);
        assert_eq!(grid[(0, 2)].id, 5);

        drop(grid);
        assert_eq!(
            drops.load(AtomicOrdering::SeqCst),
            6,
            "partial DrainCol consumption must still drop each original value exactly once"
        );
    }

    // Scenario 6: dimension invariants and basic view/iterator behavior.
    {
        // Empty array must have zero dimensions.
        let empty: TooDee<u32> = TooDee::new(0, 0);
        assert!(empty.is_empty());
        assert_eq!(empty.size(), (0, 0));

        // One dimension zero and the other non-zero must panic.
        let bad = catch_unwind(AssertUnwindSafe(|| {
            let _ = TooDee::<u32>::new(0, 1);
        }));
        assert!(bad.is_err());

        // from_vec length mismatch must panic.
        let bad2 = catch_unwind(AssertUnwindSafe(|| {
            let _ = TooDee::from_vec(2, 2, vec![1u32, 2, 3]);
        }));
        assert!(bad2.is_err());

        // Views that collapse either dimension must become empty (0,0).
        let grid = TooDee::init(3, 2, 7u32);
        let v = grid.view((1, 0), (1, 2));
        assert!(v.is_empty());
        assert_eq!(v.size(), (0, 0));

        // Iterator sizing sanity.
        assert_eq!(grid.rows().len(), 2);
        assert_eq!(grid.cells().len(), 6);
        assert_eq!(grid.col(0).len(), 2);
    }
}
