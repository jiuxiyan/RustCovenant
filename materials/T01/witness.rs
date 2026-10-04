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

fn silence_panic_output() {
    // Keep diagnostics visible; caught panics are expected.
}
fn main() {
    silence_panic_output();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut grid = TooDee::from_vec(
        2,
        1,
        vec![Datum::new(0, Arc::clone(&drops)), Datum::new(1, Arc::clone(&drops))],
    );
    let lying = WrongLengthIter {
        declared: 2,
        items: vec![Datum::new(2, Arc::clone(&drops))],
        taken: false,
    };
    let result = catch_unwind(AssertUnwindSafe(|| grid.insert_row(1, lying)));
    match &result {
        Ok(()) => {}
        Err(_) => {}
    }
    drop(grid);
    assert!(result.is_err(), "short iterator must be rejected");
    assert!(drops.load(Ordering::SeqCst) <= 3);
}
