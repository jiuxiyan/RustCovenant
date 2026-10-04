use slab::{GetDisjointMutError, Slab};
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

fn main() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut slab = Slab::new();
    slab.insert(Datum::new(10, Arc::clone(&drops)));
    slab.insert(Datum::new(20, Arc::clone(&drops)));
    slab.insert(Datum::new(30, Arc::clone(&drops)));
    assert_eq!(slab.len(), 3);
    slab.reserve(4);
    assert!(slab.capacity() >= 7);

    // Keys at or beyond entries.len are out of bounds even if capacity exists.
    assert!(matches!(
        slab.get_disjoint_mut([3]),
        Err(GetDisjointMutError::IndexOutOfBounds)
    ));
    assert!(matches!(
        slab.get_disjoint_mut([slab.capacity()]),
        Err(GetDisjointMutError::IndexOutOfBounds)
    ));

    let removed = slab.remove(1);
    assert_eq!(removed.id, 20);
    let [r0, r2] = slab.get_disjoint_mut([0, 2]).unwrap();
    r0.id = 11;
    r2.id = 33;
    let reused_or_new = slab.insert(Datum::new(40, Arc::clone(&drops)));
    assert_eq!(slab.get(reused_or_new).unwrap().id, 40);
    assert_eq!(slab.get(0).unwrap().id, 11);
    assert_eq!(slab.get(2).unwrap().id, 33);
    drop(removed);
    assert!(slab.get_disjoint_mut::<0>([]).is_ok());
    drop(slab);
    assert_eq!(drops.load(Ordering::SeqCst), 4);
}
