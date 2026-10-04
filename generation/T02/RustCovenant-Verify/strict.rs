use slab::{GetDisjointMutError, Slab};
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

fn scenario_uninitialized_index_is_oob() {
    // Contract: capacity does not imply initialized entries.
    // Requesting a key where the entry is not initialized must be IndexOutOfBounds.
    let mut slab: Slab<i32> = Slab::with_capacity(4);
    assert!(slab.capacity() >= 4);
    assert_eq!(slab.len(), 0);

    let res = slab.get_disjoint_mut([0usize; 1]);
    assert!(matches!(res, Err(GetDisjointMutError::IndexOutOfBounds)));
    assert_eq!(slab.len(), 0);
}

fn scenario_vacant_is_index_vacant() {
    let mut slab: Slab<i32> = Slab::new();
    let k0 = slab.insert(10);
    let _k1 = slab.insert(20);

    assert_eq!(slab.remove(k0), 10);
    assert_eq!(slab.get(k0), None);

    let res = slab.get_disjoint_mut([k0; 1]);
    assert!(matches!(res, Err(GetDisjointMutError::IndexVacant)));
}

fn scenario_overlapping_indices_error() {
    let mut slab: Slab<i32> = Slab::new();
    let k = slab.insert(1);

    let res = slab.get_disjoint_mut([k, k]);
    assert!(matches!(res, Err(GetDisjointMutError::OverlappingIndices)));

    // Ensure the slab remains usable and unchanged.
    assert_eq!(slab.get(k), Some(&1));
}

fn scenario_disjoint_order_and_independence() {
    // Contract: for distinct occupied keys, return mutable references in input order,
    // and writing through one reference must not affect others.
    let mut slab: Slab<i32> = Slab::new();
    let k0 = slab.insert(10);
    let k1 = slab.insert(20);
    let k2 = slab.insert(30);

    let refs = slab.get_disjoint_mut([k2, k0]).expect("keys must be occupied and disjoint");
    let [r_first, r_second] = refs;

    *r_first = 300;  // should update key k2
    *r_second = 100; // should update key k0

    assert_eq!(slab[k2], 300);
    assert_eq!(slab[k0], 100);
    assert_eq!(slab[k1], 20);
}

fn scenario_remove_reinsert_preserves_and_drops_once() {
    // Contract: removal and reinsertion preserve unaffected values and ordinary exactly-once destruction.
    let drops = Arc::new(AtomicUsize::new(0));
    let mut slab: Slab<Datum> = Slab::new();

    let k1 = slab.insert(Datum::new(1, drops.clone()));
    let k2 = slab.insert(Datum::new(2, drops.clone()));
    let k3 = slab.insert(Datum::new(3, drops.clone()));

    // Remove one value and drop it explicitly.
    let removed = slab.remove(k2);
    assert_eq!(removed.id, 2);
    drop(removed);
    assert_eq!(drops.load(Ordering::SeqCst), 1);

    // Unaffected values remain present and unchanged.
    assert_eq!(slab.len(), 2);
    assert!(slab.contains(k1));
    assert!(slab.contains(k3));
    assert_eq!(slab[k1].id, 1);
    assert_eq!(slab[k3].id, 3);

    // Reinsertion should reuse the freed key (documented key reuse behavior).
    let k4 = slab.insert(Datum::new(4, drops.clone()));
    assert_eq!(k4, k2);
    assert_eq!(slab[k4].id, 4);

    // Dropping the slab drops the remaining 3 values exactly once each.
    drop(slab);
    assert_eq!(drops.load(Ordering::SeqCst), 4);
}

fn main() {
    scenario_uninitialized_index_is_oob();
    scenario_vacant_is_index_vacant();
    scenario_overlapping_indices_error();
    scenario_disjoint_order_and_independence();
    scenario_remove_reinsert_preserves_and_drops_once();
}
