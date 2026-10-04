use stable_vec::StableVec;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct Tracked {
    id: usize,
    drops: Arc<Vec<AtomicUsize>>,
    panic_on_drop: bool,
}

impl Tracked {
    fn new(id: usize, drops: Arc<Vec<AtomicUsize>>, panic_on_drop: bool) -> Self {
        Self { id, drops, panic_on_drop }
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops[self.id].fetch_add(1, Ordering::SeqCst);
        if self.panic_on_drop {
            panic!("controlled drop failure for {}", self.id);
        }
    }
}

fn counters(n: usize) -> Arc<Vec<AtomicUsize>> {
    Arc::new((0..n).map(|_| AtomicUsize::new(0)).collect())
}

fn load(d: &Arc<Vec<AtomicUsize>>, id: usize) -> usize {
    d[id].load(Ordering::SeqCst)
}

fn assert_panics<F: FnOnce()>(f: F) {
    assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
}

fn scenario_normal_clear_and_reuse() {
    let drops = counters(4);

    let mut sv: StableVec<Tracked> = StableVec::with_capacity(10);
    assert!(sv.capacity() >= 10);

    let i0 = sv.push(Tracked::new(0, Arc::clone(&drops), false));
    let i1 = sv.push(Tracked::new(1, Arc::clone(&drops), false));
    let i2 = sv.push(Tracked::new(2, Arc::clone(&drops), false));
    assert_eq!((i0, i1, i2), (0, 1, 2));
    assert_eq!(sv.num_elements(), 3);
    assert_eq!(sv.next_push_index(), 3);

    let cap_before = sv.capacity();
    sv.clear();

    // C0: clear sets both counters to zero and retains capacity.
    assert_eq!(sv.num_elements(), 0);
    assert_eq!(sv.next_push_index(), 0);
    assert_eq!(sv.capacity(), cap_before);

    // C0: clear runs each destructor exactly once (normal path).
    assert_eq!(load(&drops, 0), 1);
    assert_eq!(load(&drops, 1), 1);
    assert_eq!(load(&drops, 2), 1);
    assert_eq!(load(&drops, 3), 0);

    // C0: reuse after clear.
    let idx = sv.push(Tracked::new(3, Arc::clone(&drops), false));
    assert_eq!(idx, 0);
    assert_eq!(sv.get(0).unwrap().id, 3);

    // Removing the last inserted element should drop it once.
    let removed = sv.remove(0).expect("element must exist");
    drop(removed);
    assert_eq!(load(&drops, 3), 1);

    // Clearing an already-empty stable vec should be fine.
    sv.clear();
    assert_eq!(sv.num_elements(), 0);
    assert_eq!(sv.next_push_index(), 0);

    // Dropping an empty StableVec should not drop anything else.
    drop(sv);
    assert_eq!(load(&drops, 0), 1);
    assert_eq!(load(&drops, 1), 1);
    assert_eq!(load(&drops, 2), 1);
    assert_eq!(load(&drops, 3), 1);
}

fn scenario_sparse_indices_survive_removals_and_inserts() {
    let mut sv: StableVec<u32> = StableVec::new();

    let a = sv.push(10);
    let b = sv.push(11);
    let c = sv.push(12);
    assert_eq!((a, b, c), (0, 1, 2));
    assert_eq!(sv.num_elements(), 3);
    assert_eq!(sv.next_push_index(), 3);

    assert_eq!(sv.remove(a), Some(10));
    assert_eq!(sv.num_elements(), 2);

    // C0: stable indices survive unrelated removals.
    assert_eq!(sv.get(b), Some(&11));
    assert_eq!(sv.get(c), Some(&12));

    // Insert into empty slot returns None and increases num_elements.
    assert_eq!(sv.insert(a, 99), None);
    assert_eq!(sv.get(a), Some(&99));
    assert_eq!(sv.num_elements(), 3);

    // Insert into filled slot replaces and returns old value.
    assert_eq!(sv.insert(b, 77), Some(11));
    assert_eq!(sv.get(b), Some(&77));
    assert_eq!(sv.num_elements(), 3);

    // Reserve for a high index and insert there adjusts next_push_index.
    sv.reserve_for(5);
    assert!(sv.capacity() >= 6);
    assert_eq!(sv.insert(5, 55), None);
    assert_eq!(sv.get(5), Some(&55));
    assert_eq!(sv.next_push_index(), 6);
    assert_eq!(sv.num_elements(), 4);

    // Out-of-bounds insert/remove must panic (documented safe API behavior).
    let cap = sv.capacity();
    assert_panics(|| {
        let _ = sv.remove(cap);
    });
    assert_panics(|| {
        let _ = sv.insert(cap, 123);
    });
}

fn scenario_panicking_drop_clear_does_not_double_drop_or_leave_dropped_reachable() {
    let drops = counters(5);
    let mut sv: StableVec<Tracked> = StableVec::new();

    // Ensure enough slots exist for indices 0..4.
    sv.reserve_for(4);
    assert!(sv.capacity() >= 5);

    // Put a panicking element early to force clear() to unwind mid-way.
    sv.push(Tracked::new(0, Arc::clone(&drops), false));
    sv.push(Tracked::new(1, Arc::clone(&drops), true));
    sv.push(Tracked::new(2, Arc::clone(&drops), false));
    sv.push(Tracked::new(3, Arc::clone(&drops), false));
    sv.push(Tracked::new(4, Arc::clone(&drops), false));
    assert_eq!(sv.num_elements(), 5);

    let r = catch_unwind(AssertUnwindSafe(|| sv.clear()));
    assert!(r.is_err());

    // After catching, occupancy must agree with num_elements (C0 panic-path).
    let cap = sv.capacity();
    let mut occupied = Vec::new();
    for i in 0..cap {
        if sv.has_element_at(i) {
            occupied.push(i);
        }
    }
    assert_eq!(occupied.len(), sv.num_elements());

    // No already-dropped value may remain reachable (C0 panic-path).
    for &i in &occupied {
        let e = sv.get(i).expect("has_element_at implies get returns Some");
        assert_eq!(load(&drops, e.id), 0, "reachable element {} was already dropped", e.id);
    }

    // Clean up remaining elements deterministically, catching only expected panics.
    for &i in &occupied {
        let e = sv.remove(i).expect("occupied slot must be removable");
        let _ = catch_unwind(AssertUnwindSafe(|| drop(e)));
    }
    assert_eq!(sv.num_elements(), 0);
    for i in 0..cap {
        assert!(!sv.has_element_at(i));
    }

    // Dropping now must not double-drop anything.
    drop(sv);
    for id in 0..5 {
        assert!(load(&drops, id) <= 1, "id {} dropped more than once", id);
    }
}

fn main() {
    scenario_normal_clear_and_reuse();
    scenario_sparse_indices_survive_removals_and_inserts();
    scenario_panicking_drop_clear_does_not_double_drop_or_leave_dropped_reachable();
}
