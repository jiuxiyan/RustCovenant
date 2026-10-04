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
    let k0 = slab.insert(Datum::new(10, Arc::clone(&drops)));
    let k1 = slab.insert(Datum::new(20, Arc::clone(&drops)));
    let k2 = slab.insert(Datum::new(30, Arc::clone(&drops)));

    let [r0, r1, r2] = slab.get_disjoint_mut([k0, k1, k2]).unwrap();
    r0.id = 100;
    r1.id = 200;
    r2.id = 300;
    assert_eq!(slab.get(0).unwrap().id, 100);
    assert_eq!(slab.get(1).unwrap().id, 200);
    assert_eq!(slab.get(2).unwrap().id, 300);

    assert!(matches!(
        slab.get_disjoint_mut([0, 0]),
        Err(GetDisjointMutError::OverlappingIndices)
    ));

    let removed = slab.remove(k1);
    assert_eq!(removed.id, 200);
    assert!(matches!(
        slab.get_disjoint_mut([1]),
        Err(GetDisjointMutError::IndexVacant)
    ));
    assert_eq!(slab.len(), 2);
    drop(removed);
    drop(slab);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}
