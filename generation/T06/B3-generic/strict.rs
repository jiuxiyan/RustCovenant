use aligned_box::{AlignedBox, AlignedBoxError};
use std::cell::{Cell, RefCell};
use std::panic::{catch_unwind, AssertUnwindSafe};

thread_local! {
    static NEXT_ID: Cell<usize> = Cell::new(0);
    static DROPS: RefCell<Vec<usize>> = RefCell::new(vec![0; 256]);
    static BOMB: Cell<Option<usize>> = Cell::new(None);
}

fn reset(bomb: Option<usize>) {
    NEXT_ID.with(|c| c.set(0));
    DROPS.with(|d| *d.borrow_mut() = vec![0; 256]);
    BOMB.with(|b| b.set(bomb));
}

fn next_id() -> usize {
    NEXT_ID.with(|c| {
        let id = c.get();
        c.set(id + 1);
        id
    })
}

fn drop_count(id: usize) -> usize {
    DROPS.with(|d| d.borrow().get(id).copied().unwrap_or(0))
}

#[derive(Debug)]
struct Track {
    id: usize,
    payload: String,
}

impl Default for Track {
    fn default() -> Self {
        let id = next_id();
        Track {
            id,
            payload: format!("item{}", id),
        }
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        DROPS.with(|d| {
            let mut v = d.borrow_mut();
            if self.id >= v.len() {
                v.resize(self.id + 1, 0);
            }
            v[self.id] += 1;
        });

        let fire = BOMB.with(|b| {
            if b.get() == Some(self.id) {
                b.set(None);
                true
            } else {
                false
            }
        });

        if fire {
            panic!("injected destructor panic");
        }
    }
}

fn test_errors_invalid_align_and_zero_alloc() {
    let e = AlignedBox::<u8>::new(3, 1).unwrap_err();
    assert!(matches!(e, AlignedBoxError::InvalidAlign));

    let e = AlignedBox::<[u8]>::slice_from_value(3, 4, 9).unwrap_err();
    assert!(matches!(e, AlignedBoxError::InvalidAlign));

    let e = AlignedBox::<[u8]>::slice_from_default(3, 4).unwrap_err();
    assert!(matches!(e, AlignedBoxError::InvalidAlign));

    let e = AlignedBox::<[u8]>::slice_from_value(8, 0, 1).unwrap_err();
    assert!(matches!(e, AlignedBoxError::ZeroAlloc));

    let e = AlignedBox::<[u8]>::slice_from_default(8, 0).unwrap_err();
    assert!(matches!(e, AlignedBoxError::ZeroAlloc));

    let e = AlignedBox::<()>::new(8, ()).unwrap_err();
    assert!(matches!(e, AlignedBoxError::ZeroAlloc));
}

fn test_min_alignment_is_enforced_for_new() {
    #[repr(align(32))]
    struct A(u8);

    let b = AlignedBox::new(1, A(7)).unwrap();
    let addr = (&*b as *const A) as usize;
    assert_eq!(addr % 32, 0);
    assert_eq!(((*b).0), 7);
}

fn test_clone_deep_copy_semantics() {
    let mut b = AlignedBox::new(64, vec![1u8, 2, 3]).unwrap();
    let mut c = b.clone();
    assert_eq!(*b, *c);

    b[0] = 8;
    c[2] = 9;

    assert_eq!(b[0], 8);
    assert_eq!(b[2], 3);
    assert_eq!(c[0], 1);
    assert_eq!(c[2], 9);
}

fn test_slice_value_realloc_preserves_prefix_and_alignment() {
    let align = 64;
    let mut b = AlignedBox::<[u32]>::slice_from_value(align, 5, 7).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 5);
    for i in 0..5 {
        assert_eq!(b[i], 7);
    }

    b.realloc_with_value(8, 9).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 8);
    for i in 0..5 {
        assert_eq!(b[i], 7);
    }
    for i in 5..8 {
        assert_eq!(b[i], 9);
    }

    b.realloc_with_value(3, 1).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 3);
    for i in 0..3 {
        assert_eq!(b[i], 7);
    }
}

fn test_realloc_default_success_preserves_prefix_inits_new_and_drops_removed_once() {
    reset(None);
    let align = 64;

    let mut b = AlignedBox::<[Track]>::slice_from_default(align, 5).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 5);

    for i in 0..5 {
        assert_eq!(b[i].id, i);
        assert_eq!(b[i].payload, format!("item{}", i));
        assert_eq!(drop_count(i), 0);
    }

    // Mutate some payload to ensure the prefix really stays intact.
    b[1].payload.push_str("_x");
    b[4].payload = "custom".to_string();

    // Grow: prefix preserved, new elements default-initialized, alignment preserved.
    b.realloc_with_default(8).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 8);

    assert_eq!(b[1].payload, "item1_x");
    assert_eq!(b[4].payload, "custom");

    for i in 0..5 {
        assert_eq!(b[i].id, i);
        assert_eq!(drop_count(i), 0);
    }
    for i in 5..8 {
        assert_eq!(b[i].id, i);
        assert_eq!(b[i].payload, format!("item{}", i));
        assert_eq!(drop_count(i), 0);
    }

    // Shrink: removed elements dropped exactly once, prefix preserved, alignment preserved.
    b.realloc_with_default(3).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 3);

    for i in 0..3 {
        assert_eq!(drop_count(i), 0);
        assert_eq!(b[i].id, i);
    }
    for i in 3..8 {
        assert_eq!(drop_count(i), 1);
    }

    drop(b);
    for i in 0..8 {
        assert_eq!(drop_count(i), 1);
    }
}

fn test_shrink_destructor_panic_does_not_cause_double_drop_or_invalid_drop() {
    reset(Some(4));
    let align = 64;

    let mut b = AlignedBox::<[Track]>::slice_from_default(align, 6).unwrap();
    assert_eq!((b.as_ptr() as usize) % align, 0);
    assert_eq!(b.len(), 6);

    // Shrink such that id=4 is in the removed tail, so its Drop panics.
    let r = catch_unwind(AssertUnwindSafe(|| {
        let _ = b.realloc_with_default(2);
    }));
    assert!(r.is_err());

    // The panicking destructor must have run exactly once.
    assert_eq!(drop_count(4), 1);

    // Dropping the box must not revisit already-dropped elements or use invalid metadata.
    drop(b);

    for id in 0..6 {
        assert!(drop_count(id) <= 1, "id {id} dropped more than once");
    }
}

fn main() {
    test_errors_invalid_align_and_zero_alloc();
    test_min_alignment_is_enforced_for_new();
    test_clone_deep_copy_semantics();
    test_slice_value_realloc_preserves_prefix_and_alignment();
    test_realloc_default_success_preserves_prefix_inits_new_and_drops_removed_once();
    test_shrink_destructor_panic_does_not_cause_double_drop_or_invalid_drop();
}
