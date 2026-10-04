use toodee::{TooDee, TooDeeOps};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct Datum {
    id: u32,
    drops: Arc<AtomicUsize>,
}

impl Datum {
    fn new(id: u32, drops: Arc<AtomicUsize>) -> Self {
        Self { id, drops }
    }
}

impl Drop for Datum {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

// ExactSizeIterator that lies: declares N but yields fewer.
struct WrongLengthIter {
    declared: usize,
    items: Vec<Datum>,
    taken: bool,
}

impl Iterator for WrongLengthIter {
    type Item = Datum;
    fn next(&mut self) -> Option<Datum> {
        if self.taken {
            None
        } else {
            self.taken = true;
            self.items.pop()
        }
    }
}

impl ExactSizeIterator for WrongLengthIter {
    fn len(&self) -> usize {
        self.declared
    }
}

// ExactSizeIterator that panics on the second next().
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
        2
    }
}

fn assert_row_ids(grid: &TooDee<Datum>, row: usize, expected: &[u32]) {
    let r = &grid[row];
    assert_eq!(r.len(), expected.len());
    for (i, &eid) in expected.iter().enumerate() {
        assert_eq!(r[i].id, eid, "row {row} col {i}");
    }
}

fn scenario_normal_insert_row_middle() {
    let drops = Arc::new(AtomicUsize::new(0));

    // 3x2, row-major ids 0..=5
    let mut grid = TooDee::from_vec(
        3,
        2,
        vec![
            Datum::new(0, Arc::clone(&drops)),
            Datum::new(1, Arc::clone(&drops)),
            Datum::new(2, Arc::clone(&drops)),
            Datum::new(3, Arc::clone(&drops)),
            Datum::new(4, Arc::clone(&drops)),
            Datum::new(5, Arc::clone(&drops)),
        ],
    );

    let inserted = vec![
        Datum::new(100, Arc::clone(&drops)),
        Datum::new(101, Arc::clone(&drops)),
        Datum::new(102, Arc::clone(&drops)),
    ];

    grid.insert_row(1, inserted.into_iter());

    assert_eq!(grid.num_cols(), 3);
    assert_eq!(grid.num_rows(), 3);
    assert_row_ids(&grid, 0, &[0, 1, 2]);
    assert_row_ids(&grid, 1, &[100, 101, 102]);
    assert_row_ids(&grid, 2, &[3, 4, 5]);

    drop(grid);
    // Normal completion: all owned values must be dropped exactly once.
    assert_eq!(drops.load(Ordering::SeqCst), 9);
}

fn scenario_short_iterator_rejected_no_double_drop() {
    let drops = Arc::new(AtomicUsize::new(0));

    let mut grid = TooDee::from_vec(
        2,
        1,
        vec![
            Datum::new(0, Arc::clone(&drops)),
            Datum::new(1, Arc::clone(&drops)),
        ],
    );

    // Declares 2 (matching num_cols), but yields only 1.
    let lying = WrongLengthIter {
        declared: 2,
        items: vec![Datum::new(2, Arc::clone(&drops))],
        taken: false,
    };

    let result = catch_unwind(AssertUnwindSafe(|| {
        grid.insert_row(1, lying);
    }));

    assert!(result.is_err(), "malformed short iterator must be rejected");

    drop(grid);

    // Panic-path: leaks permitted, but duplicate destruction must not occur.
    // We created exactly 3 Datum instances.
    let observed = drops.load(Ordering::SeqCst);
    assert!(observed <= 3, "drops must not exceed creations (got {observed})");
}

fn scenario_iterator_panics_no_double_drop() {
    let drops = Arc::new(AtomicUsize::new(0));

    let mut grid = TooDee::from_vec(
        2,
        1,
        vec![
            Datum::new(0, Arc::clone(&drops)),
            Datum::new(1, Arc::clone(&drops)),
        ],
    );

    let panicky = PanicAfterOne {
        first: Some(Datum::new(2, Arc::clone(&drops))),
    };

    let result = catch_unwind(AssertUnwindSafe(|| {
        grid.insert_row(1, panicky);
    }));

    assert!(result.is_err(), "iterator panic must propagate");

    drop(grid);

    // Panic-path: leaks permitted, but duplicate destruction must not occur.
    let observed = drops.load(Ordering::SeqCst);
    assert!(observed <= 3, "drops must not exceed creations (got {observed})");
}

fn main() {
    scenario_normal_insert_row_middle();
    scenario_short_iterator_rejected_no_double_drop();
    scenario_iterator_panics_no_double_drop();
}
