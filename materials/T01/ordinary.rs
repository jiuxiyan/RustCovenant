use toodee::{TooDee, TooDeeOps};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Datum {
    pub id: u32,
    pub drops: Arc<AtomicUsize>,
}

impl Datum {
    pub fn new(id: u32, drops: Arc<AtomicUsize>) -> Self {
        Self { id, drops }
    }
}

impl Drop for Datum {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

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

struct PanicAfterOne {
    first: Option<Datum>,
}

impl Iterator for PanicAfterOne {
    type Item = Datum;
    fn next(&mut self) -> Option<Datum> {
        if let Some(value) = self.first.take() {
            Some(value)
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

fn main() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut grid = TooDee::from_vec(
        2,
        2,
        vec![
            Datum::new(0, Arc::clone(&drops)),
            Datum::new(1, Arc::clone(&drops)),
            Datum::new(2, Arc::clone(&drops)),
            Datum::new(3, Arc::clone(&drops)),
        ],
    );
    grid.insert_row(
        1,
        vec![Datum::new(10, Arc::clone(&drops)), Datum::new(11, Arc::clone(&drops))],
    );
    assert_eq!(grid.num_cols(), 2);
    assert_eq!(grid.num_rows(), 3);
    assert_eq!(grid.size(), (2, 3));
    assert_eq!(grid[0][0].id, 0);
    assert_eq!(grid[0][1].id, 1);
    assert_eq!(grid[1][0].id, 10);
    assert_eq!(grid[1][1].id, 11);
    assert_eq!(grid[2][0].id, 2);
    assert_eq!(grid[2][1].id, 3);
    drop(grid);
    assert_eq!(drops.load(Ordering::SeqCst), 6);
}
