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
    let mut slab: Slab<Datum> = Slab::with_capacity(4);
    assert!(slab.capacity() >= 4);
    assert_eq!(slab.len(), 0);
    let result = slab.get_disjoint_mut([0usize; 1]);
    assert!(matches!(result, Err(GetDisjointMutError::IndexOutOfBounds)));
    drop(slab);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}
