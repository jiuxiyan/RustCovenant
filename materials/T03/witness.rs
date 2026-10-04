use stable_vec::StableVec;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Datum {
    pub id: u32,
    pub drops: Arc<AtomicUsize>,
    pub panic_on_drop: bool,
}

impl Datum {
    pub fn new(id: u32, drops: Arc<AtomicUsize>, panic_on_drop: bool) -> Self {
        Self { id, drops, panic_on_drop }
    }
}

impl Drop for Datum {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        if self.panic_on_drop {
            panic!("controlled drop failure for {}", self.id);
        }
    }
}

fn silence_panic_output() {
    // Keep diagnostics visible; caught panics are expected.
}
fn main() {
    silence_panic_output();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut sv = StableVec::new();
    sv.push(Datum::new(1, Arc::clone(&drops), false));
    sv.push(Datum::new(2, Arc::clone(&drops), true));
    sv.push(Datum::new(3, Arc::clone(&drops), false));

    let result = catch_unwind(AssertUnwindSafe(|| sv.clear()));
    assert!(result.is_err());
    drop(sv);
    assert!(drops.load(Ordering::SeqCst) <= 3);
}
