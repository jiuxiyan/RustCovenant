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

    let mut nums: StableVec<u32> = StableVec::from(&[10u32, 20, 30]);
    assert_eq!(nums.num_elements(), 3);
    nums.clear();
    assert_eq!(nums.num_elements(), 0);
    assert!(nums.is_empty());
    assert_eq!(nums.next_push_index(), 0);
    assert!(nums.capacity() >= 3);
    assert_eq!(nums.push(40), 0);
    assert_eq!(nums.get(0), Some(&40));

    let drops = Arc::new(AtomicUsize::new(0));
    let mut sv = StableVec::new();
    sv.push(Datum::new(1, Arc::clone(&drops), false));
    sv.push(Datum::new(2, Arc::clone(&drops), false));
    sv.push(Datum::new(3, Arc::clone(&drops), false));
    sv.clear();
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    sv.push(Datum::new(4, Arc::clone(&drops), false));
    drop(sv);
    assert_eq!(drops.load(Ordering::SeqCst), 4);

    // Stable indices and holes retain their documented semantics.
    let drops2 = Arc::new(AtomicUsize::new(0));
    let mut holes = StableVec::new();
    let k0 = holes.push(Datum::new(10, Arc::clone(&drops2), false));
    let k1 = holes.push(Datum::new(11, Arc::clone(&drops2), false));
    let k2 = holes.push(Datum::new(12, Arc::clone(&drops2), false));
    let removed = holes.remove(k1).unwrap();
    assert_eq!(removed.id, 11);
    drop(removed);
    let k3 = holes.push(Datum::new(13, Arc::clone(&drops2), false));
    assert_eq!((k0, k2, k3), (0, 2, 3));
    assert!(holes.has_element_at(0));
    assert!(!holes.has_element_at(1));
    assert!(holes.has_element_at(2));
    assert!(holes.has_element_at(3));
    holes.clear();
    assert_eq!(drops2.load(Ordering::SeqCst), 4);
}
