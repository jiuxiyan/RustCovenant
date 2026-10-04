use stable_vec::{InlineStableVec, StableVec, StableVecFacade};
use stable_vec::core::Core;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn assert_panics<F: FnOnce() -> R, R>(f: F) {
    assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
}

fn assert_ok<F: FnOnce() -> R, R>(f: F) {
    assert!(catch_unwind(AssertUnwindSafe(f)).is_ok());
}

fn assert_invariants<T, C: Core<T>>(sv: &StableVecFacade<T, C>) {
    let n = sv.num_elements();
    assert_eq!(sv.indices().count(), n);
    assert_eq!(sv.iter().count(), n);
    assert_eq!(sv.values().count(), n);
    assert_eq!(sv.is_empty(), n == 0);
    assert_eq!(sv.is_compact(), sv.next_push_index() == n);

    // Basic consistency between `get` and `has_element_at`.
    // (Only checks within capacity to avoid relying on any layout outside.)
    for i in 0..sv.capacity() {
        assert_eq!(sv.get(i).is_some(), sv.has_element_at(i));
    }

    // Indices must be strictly increasing.
    let mut last: Option<usize> = None;
    for idx in sv.indices() {
        if let Some(prev) = last {
            assert!(prev < idx);
        }
        last = Some(idx);
    }
}

#[derive(Debug)]
struct Datum {
    id: u32,
    drops: Arc<AtomicUsize>,
    panic_on_drop: bool,
}

impl Datum {
    fn new(id: u32, drops: Arc<AtomicUsize>, panic_on_drop: bool) -> Self {
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

fn scenario_basic_sparse_insert_remove() {
    let mut sv = StableVec::<i32>::new();
    assert_eq!(sv.capacity(), 0);
    assert_eq!(sv.num_elements(), 0);
    assert_eq!(sv.next_push_index(), 0);

    // Reserve to create empty slots; no elements inserted.
    sv.reserve_for(3);
    assert!(sv.capacity() >= 4);
    assert_eq!(sv.num_elements(), 0);
    assert_eq!(sv.next_push_index(), 0);
    assert_invariants(&sv);

    // Insert into a reserved slot beyond current next_push_index.
    assert_eq!(sv.insert(3, 30), None);
    assert_eq!(sv.get(3), Some(&30));
    assert_eq!(sv.num_elements(), 1);
    assert_eq!(sv.next_push_index(), 4);
    assert_invariants(&sv);

    // Removing does not shift; next_push_index stays.
    assert_eq!(sv.remove(3), Some(30));
    assert_eq!(sv.remove(3), None);
    assert_eq!(sv.num_elements(), 0);
    assert_eq!(sv.next_push_index(), 4);
    assert_invariants(&sv);

    // Push uses next_push_index even if earlier holes exist.
    let idx = sv.push(7);
    assert_eq!(idx, 4);
    assert_eq!(sv.get(4), Some(&7));
    assert_eq!(sv.num_elements(), 1);
    assert_eq!(sv.next_push_index(), 5);

    // Fill an earlier hole.
    assert_eq!(sv.insert(1, 11), None);
    assert_eq!(sv.get(1), Some(&11));
    assert_eq!(sv.num_elements(), 2);
    assert_eq!(sv.next_push_index(), 5);
    assert_invariants(&sv);
}

fn scenario_iterators_and_search() {
    let mut sv = StableVec::from(&[10, 11, 12, 13, 14]);
    assert_eq!(sv.num_elements(), 5);
    sv.remove(1);
    sv.remove(3);
    assert_eq!(sv.num_elements(), 3);

    let got: Vec<(usize, i32)> = sv.iter().map(|(i, v)| (i, *v)).collect();
    assert_eq!(got, vec![(0, 10), (2, 12), (4, 14)]);

    let mut ind = sv.indices();
    assert_eq!(ind.size_hint(), (3, Some(3)));
    assert_eq!(ind.next(), Some(0));
    assert_eq!(ind.next(), Some(2));
    assert_eq!(ind.next(), Some(4));
    assert_eq!(ind.next(), None);

    // Search helpers: allow start == capacity; start > capacity panics.
    assert_eq!(sv.first_filled_slot_from(0), Some(0));
    assert_eq!(sv.first_filled_slot_from(1), Some(2));
    assert_eq!(sv.first_filled_slot_from(sv.capacity()), None);
    assert_panics(|| {
        let _ = sv.first_filled_slot_from(sv.capacity() + 1);
    });

    assert_eq!(sv.first_empty_slot_from(0), Some(1));
    assert_eq!(sv.first_empty_slot_below(0), None);

    assert_invariants(&sv);
}

fn scenario_swap_behavior() {
    let mut sv = StableVec::from(&[1, 2, 3, 4]);
    sv.reserve_for(5);
    assert!(sv.capacity() >= 6);
    assert_eq!(sv.next_push_index(), 4);

    // Swap filled with empty.
    sv.swap(0, 5);
    assert_eq!(sv.get(0), None);
    assert_eq!(sv.get(5), Some(&1));
    assert_eq!(sv.next_push_index(), 6);

    // Swap filled with filled.
    sv.swap(1, 2);
    assert_eq!(sv.get(1), Some(&3));
    assert_eq!(sv.get(2), Some(&2));

    // Swap empty with empty: no observable element change.
    sv.swap(0, 4);
    assert_eq!(sv.get(0), None);
    assert_eq!(sv.get(4), None);

    // Out-of-bounds swap must panic.
    let cap = sv.capacity();
    assert_panics(|| sv.swap(cap, 0));

    assert_invariants(&sv);
}

fn scenario_compaction_and_shrink() {
    let mut sv = StableVec::from(&[0, 1, 2, 3, 4, 5]);
    let cap_before = sv.capacity();
    sv.remove(1);
    sv.remove(4);
    assert!(!sv.is_compact());

    let before_vals: Vec<i32> = sv.values().copied().collect();
    assert_eq!(before_vals, vec![0, 2, 3, 5]);

    sv.make_compact();
    assert!(sv.is_compact());
    assert_eq!(sv.num_elements(), 4);
    assert_eq!(sv.next_push_index(), 4);
    assert_eq!(sv.values().copied().collect::<Vec<_>>(), before_vals);
    assert_eq!(sv.capacity(), cap_before);

    // Shrink should not change next_push_index; capacity should not increase.
    let cap_mid = sv.capacity();
    let npi = sv.next_push_index();
    sv.shrink_to_fit();
    assert_eq!(sv.next_push_index(), npi);
    assert!(sv.capacity() >= sv.next_push_index());
    assert!(sv.capacity() <= cap_mid);

    assert_invariants(&sv);
}

fn scenario_reordering_compact_multiset() {
    let mut sv = StableVec::from(&[100, 101, 102, 103, 104, 105]);
    sv.remove(1);
    sv.remove(4);

    let mut before: Vec<i32> = sv.values().copied().collect();
    before.sort();

    sv.reordering_make_compact();
    assert!(sv.is_compact());
    assert_eq!(sv.next_push_index(), sv.num_elements());

    let mut after: Vec<i32> = sv.values().copied().collect();
    after.sort();
    assert_eq!(after, before);

    assert_invariants(&sv);
}

fn scenario_retain_and_eq_semantics() {
    let mut sv = StableVec::from(&[1, 2, 3, 4, 5, 6]);
    sv.retain(|&e| e % 2 == 0);
    assert_eq!(sv.values().copied().collect::<Vec<_>>(), vec![2, 4, 6]);
    assert_eq!(sv, vec![2, 4, 6]);

    let mut sv2 = StableVec::new();
    let i0 = sv2.push(10);
    let i1 = sv2.push(20);
    let i2 = sv2.push(30);
    assert_eq!((i0, i1, i2), (0, 1, 2));
    sv2.retain_indices(|idx| idx == i1);
    assert_eq!(sv2.num_elements(), 1);
    assert_eq!(sv2.get(i1), Some(&20));
    assert_eq!(sv2.get(i0), None);
    assert_eq!(sv2.get(i2), None);

    assert_invariants(&sv);
    assert_invariants(&sv2);
}

fn scenario_panics_and_clear_contract<C>(mut sv: StableVecFacade<Datum, C>)
where
    C: Core<Datum>,
{
    // Empty methods that must panic on invalid indices/capacity.
    assert_eq!(sv.capacity(), 0);
    assert_panics(|| {
        let _ = sv.insert(0, Datum::new(999, Arc::new(AtomicUsize::new(0)), false));
    });
    assert_panics(|| {
        let _ = sv.remove(0);
    });

    // Indexing an empty slot must panic.
    assert_panics(|| {
        let _ = sv[0].id;
    });

    // Now set up the controlled panicking-drop scenario.
    let drops = Arc::new(AtomicUsize::new(0));
    sv.push(Datum::new(1, Arc::clone(&drops), false));
    sv.push(Datum::new(2, Arc::clone(&drops), true));
    sv.push(Datum::new(3, Arc::clone(&drops), false));

    assert_invariants(&sv);

    // `clear` should panic due to one element's Drop.
    let r = catch_unwind(AssertUnwindSafe(|| sv.clear()));
    assert!(r.is_err());

    // After catching panic: safe queries must remain consistent.
    assert_invariants(&sv);

    // Dropping the container must not cause a second panic or double-drop.
    assert_ok(|| drop(sv));

    // No double-drop allowed; leaks are allowed.
    assert!(drops.load(Ordering::SeqCst) <= 3);
}

fn main() {
    scenario_basic_sparse_insert_remove();
    scenario_iterators_and_search();
    scenario_swap_behavior();
    scenario_compaction_and_shrink();
    scenario_reordering_compact_multiset();
    scenario_retain_and_eq_semantics();

    // Run panic/clear regression on both core variants.
    scenario_panics_and_clear_contract(StableVec::<Datum>::new());
    scenario_panics_and_clear_contract(InlineStableVec::<Datum>::new());
}
