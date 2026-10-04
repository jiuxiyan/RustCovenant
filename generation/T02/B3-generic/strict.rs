use slab::{GetDisjointMutError, Slab};
use std::panic::{catch_unwind, AssertUnwindSafe};
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

fn assert_panics<F: FnOnce() -> R, R>(f: F) {
    let r = catch_unwind(AssertUnwindSafe(f));
    assert!(r.is_err(), "expected panic");
}

fn main() {
    // 1) Regression: get_disjoint_mut must NOT touch uninitialized entries.
    // Empty slab with reserved capacity must return IndexOutOfBounds for any key.
    {
        let mut s: Slab<u32> = Slab::with_capacity(4);
        assert!(s.capacity() >= 4);
        assert_eq!(s.len(), 0);

        // Key 0 is < capacity but >= entries.len() (uninitialized). Must be out of bounds.
        let r = s.get_disjoint_mut([0usize; 1]);
        assert!(matches!(r, Err(GetDisjointMutError::IndexOutOfBounds)));

        // Another uninitialized index.
        let r = s.get_disjoint_mut([3usize; 1]);
        assert!(matches!(r, Err(GetDisjointMutError::IndexOutOfBounds)));
    }

    // 2) get_disjoint_mut: disjoint occupied keys, returned in input order, truly disjoint.
    {
        let mut s: Slab<i32> = Slab::new();
        let k0 = s.insert(10);
        let k1 = s.insert(11);
        let k2 = s.insert(12);
        let k3 = s.insert(13);
        assert_eq!((k0, k1, k2, k3), (0, 1, 2, 3));

        let [a, b, c] = s.get_disjoint_mut([2, 0, 3]).expect("must succeed");
        // Returned in the same order as requested keys.
        assert_eq!(*a, 12);
        assert_eq!(*b, 10);
        assert_eq!(*c, 13);

        // Mutating one must not affect others.
        *a += 1000;
        *b += 2000;
        *c += 3000;

        assert_eq!(s[0], 2010);
        assert_eq!(s[1], 11);
        assert_eq!(s[2], 1012);
        assert_eq!(s[3], 3013);
    }

    // 3) get_disjoint_mut error cases: overlapping indices, vacant index, out-of-bounds.
    {
        let mut s: Slab<u8> = Slab::with_capacity(8);
        let k0 = s.insert(1);
        let k1 = s.insert(2);
        let _k2 = s.insert(3);
        assert_eq!((k0, k1), (0, 1));

        // Duplicate keys -> OverlappingIndices (not panic).
        let r = s.get_disjoint_mut([1usize, 1usize]);
        assert!(matches!(r, Err(GetDisjointMutError::OverlappingIndices)));

        // Remove to create a vacancy.
        let removed = s.remove(1);
        assert_eq!(removed, 2);
        assert_eq!(s.len(), 2);
        assert!(!s.contains(1));

        // Vacant slot -> IndexVacant.
        let r = s.get_disjoint_mut([1usize; 1]);
        assert!(matches!(r, Err(GetDisjointMutError::IndexVacant)));

        // An index well beyond entries.len() must be IndexOutOfBounds even if capacity is large.
        // (This is the core uninitialized-access boundary.)
        let r = s.get_disjoint_mut([7usize; 1]);
        assert!(matches!(r, Err(GetDisjointMutError::IndexOutOfBounds)));

        // Also out-of-bounds for very large key.
        let r = s.get_disjoint_mut([9999usize; 1]);
        assert!(matches!(r, Err(GetDisjointMutError::IndexOutOfBounds)));
    }

    // 4) Removal and reinsertion: preserve unaffected values, reuse key, exactly-once drops.
    {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut s: Slab<Datum> = Slab::new();

        let k0 = s.insert(Datum::new(10, drops.clone()));
        let k1 = s.insert(Datum::new(11, drops.clone()));
        let k2 = s.insert(Datum::new(12, drops.clone()));
        assert_eq!((k0, k1, k2), (0, 1, 2));
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        // Remove one element and ensure it drops exactly once when dropped.
        let removed = s.remove(k1);
        assert_eq!(removed.id, 11);
        assert_eq!(s.len(), 2);
        assert!(s.contains(k0));
        assert!(s.contains(k2));
        assert!(!s.contains(k1));
        drop(removed);
        assert_eq!(drops.load(Ordering::SeqCst), 1);

        // vacant_key should point to the just-freed slot.
        assert_eq!(s.vacant_key(), k1);

        // Reinsertion should reuse the freed key.
        let k1b = s.insert(Datum::new(21, drops.clone()));
        assert_eq!(k1b, k1);
        assert_eq!(s.len(), 3);
        assert_eq!(s[k0].id, 10);
        assert_eq!(s[k1].id, 21);
        assert_eq!(s[k2].id, 12);

        drop(s);
        // Total drops: removed(1) + remaining in slab at drop time (3) = 4.
        assert_eq!(drops.load(Ordering::SeqCst), 4);
    }

    // 5) VacantEntry: key reservation and insertion at that key.
    {
        let mut s: Slab<(usize, &'static str)> = Slab::new();
        assert_eq!(s.vacant_key(), 0);
        let key = {
            let e = s.vacant_entry();
            let k = e.key();
            let v = e.insert((k, "hello"));
            assert_eq!(*v, (k, "hello"));
            k
        };
        assert_eq!(key, 0);
        assert_eq!(s[key], (0, "hello"));
    }

    // 6) FromIterator builder behavior: unsorted keys and duplicates replace earlier values.
    {
        let pairs = vec![(2usize, 'a'), (0usize, 'b'), (2usize, 'c')];
        let s: Slab<char> = pairs.into_iter().collect();
        assert_eq!(s.len(), 2);
        assert!(s.contains(0));
        assert!(s.contains(2));
        assert_eq!(s.get(1), None);
        assert_eq!(s[0], 'b');
        assert_eq!(s[2], 'c');

        // Iteration yields only occupied entries.
        let collected: Vec<(usize, char)> = s.iter().map(|(k, v)| (k, *v)).collect();
        assert_eq!(collected, vec![(0, 'b'), (2, 'c')]);
    }

    // 7) key_of: correct key for a reference into slab; panics for foreign references.
    {
        let mut s: Slab<String> = Slab::new();
        let k = s.insert("foo".to_string());
        let r: &String = &s[k];
        assert_eq!(s.key_of(r), k);

        let foreign = "foo".to_string();
        assert_panics(|| {
            let _ = s.key_of(&foreign);
        });
    }

    // 8) get2_mut: panics on identical keys (documented).
    {
        let mut s: Slab<i32> = Slab::new();
        let k = s.insert(1);
        assert_panics(|| {
            let _ = s.get2_mut(k, k);
        });
    }
}
