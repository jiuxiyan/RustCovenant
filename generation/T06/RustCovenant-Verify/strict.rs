use aligned_box::AlignedBox;
use std::{
    cell::{Cell, RefCell},
    panic::{catch_unwind, AssertUnwindSafe},
};

// ----------------------------
// Tracking type (non-panicking)
// ----------------------------
thread_local! {
    static NEXT_ID: Cell<usize> = Cell::new(0);
    static DROPS: RefCell<Vec<usize>> = RefCell::new(vec![0; 256]);
}

#[derive(Debug)]
struct Track {
    id: usize,
    val: usize,
}

impl Default for Track {
    fn default() -> Self {
        let id = NEXT_ID.with(|c| {
            let id = c.get();
            c.set(id + 1);
            id
        });
        Track { id, val: 0 }
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        DROPS.with(|d| {
            let mut d = d.borrow_mut();
            if self.id >= d.len() {
                d.resize(self.id + 1, 0);
            }
            d[self.id] += 1;
        });
    }
}

fn reset_track() {
    NEXT_ID.with(|c| c.set(0));
    DROPS.with(|d| *d.borrow_mut() = vec![0; 256]);
}

fn next_track_id() -> usize {
    NEXT_ID.with(|c| c.get())
}

fn assert_track_drop_exactly_once(created: usize) {
    DROPS.with(|d| {
        let d = d.borrow();
        for i in 0..created {
            assert_eq!(
                d[i], 1,
                "expected Track id {i} to be dropped exactly once, got {}",
                d[i]
            );
        }
    });
}

fn assert_track_drop_counts(prefix: &[(usize, usize)], dropped_ids_once: &[usize]) {
    // prefix: (id, expected_drop_count)
    DROPS.with(|d| {
        let d = d.borrow();
        for &(id, expected) in prefix {
            assert_eq!(d[id], expected, "Track id {id} drop count mismatch");
        }
        for &id in dropped_ids_once {
            assert_eq!(d[id], 1, "expected Track id {id} to be dropped once");
        }
    });
}

// ----------------------------
// Bomb type (Drop can panic)
// ----------------------------
thread_local! {
    static BOMB_NEXT_ID: Cell<usize> = Cell::new(0);
    static BOMB_DROPS: RefCell<Vec<usize>> = RefCell::new(vec![0; 256]);
    static BOMB_ID: Cell<Option<usize>> = Cell::new(None);
}

#[derive(Debug)]
struct BombTrack {
    id: usize,
    val: usize,
}

impl Default for BombTrack {
    fn default() -> Self {
        let id = BOMB_NEXT_ID.with(|c| {
            let id = c.get();
            c.set(id + 1);
            id
        });
        BombTrack { id, val: 0 }
    }
}

impl Drop for BombTrack {
    fn drop(&mut self) {
        BOMB_DROPS.with(|d| {
            let mut d = d.borrow_mut();
            if self.id >= d.len() {
                d.resize(self.id + 1, 0);
            }
            d[self.id] += 1;
        });

        let should_panic = BOMB_ID.with(|b| match b.get() {
            Some(id) if id == self.id => {
                b.set(None);
                true
            }
            _ => false,
        });

        if should_panic {
            panic!("injected destructor panic");
        }
    }
}

fn reset_bomb(bomb: Option<usize>) {
    BOMB_NEXT_ID.with(|c| c.set(0));
    BOMB_DROPS.with(|d| *d.borrow_mut() = vec![0; 256]);
    BOMB_ID.with(|b| b.set(bomb));
}

fn bomb_next_id() -> usize {
    BOMB_NEXT_ID.with(|c| c.get())
}

fn assert_bomb_no_double_drop(created: usize) {
    BOMB_DROPS.with(|d| {
        let d = d.borrow();
        for i in 0..created {
            assert!(d[i] <= 1, "BombTrack id {i} dropped more than once: {}", d[i]);
        }
    });
}

// ----------------------------
// Helpers
// ----------------------------
fn assert_slice_aligned<T>(b: &AlignedBox<[T]>, requested_alignment: usize) {
    let addr = b.as_ptr() as usize;
    assert_eq!(addr % requested_alignment, 0, "slice pointer not aligned as requested");
}

fn scenario_scalar_new_alignment_upgrade_and_mutation() {
    #[repr(align(128))]
    struct BigAlign(u8);

    // Requested alignment smaller than align_of::<BigAlign>() must be upgraded.
    let mut b = AlignedBox::new(1, BigAlign(7)).unwrap();
    let addr = (&*b as *const BigAlign) as usize;
    assert_eq!(addr % std::mem::align_of::<BigAlign>(), 0);

    // Exercise DerefMut (safe API usage).
    b.0 = 9;
    assert_eq!(b.0, 9);
}

fn scenario_grow_preserve_prefix_and_default_init_suffix_and_alignment_and_no_leak() {
    reset_track();

    let mut b = AlignedBox::<[Track]>::slice_from_default(64, 4).unwrap();
    assert_eq!(b.len(), 4);
    assert_slice_aligned(&b, 64);

    // Deterministic IDs given the initializer loop order.
    for (i, t) in b.iter().enumerate() {
        assert_eq!(t.id, i);
        assert_eq!(t.val, 0);
    }

    for i in 0..4 {
        b[i].val = 1000 + i;
    }

    b.realloc_with_default(7).unwrap();
    assert_eq!(b.len(), 7);
    assert_slice_aligned(&b, 64);

    // Preserve original prefix.
    for i in 0..4 {
        assert_eq!(b[i].val, 1000 + i);
        assert_eq!(b[i].id, i);
    }

    // Default-initialize new elements.
    for i in 4..7 {
        assert_eq!(b[i].val, 0);
        assert_eq!(b[i].id, i);
    }

    drop(b);
    assert_track_drop_exactly_once(next_track_id());
}

fn scenario_shrink_destroys_removed_exactly_once_and_preserves_prefix_and_alignment_and_no_leak() {
    reset_track();

    let mut b = AlignedBox::<[Track]>::slice_from_default(64, 6).unwrap();
    assert_eq!(b.len(), 6);
    assert_slice_aligned(&b, 64);

    for i in 0..6 {
        b[i].val = 2000 + i;
        assert_eq!(b[i].id, i);
    }

    let removed_ids: Vec<usize> = b.iter().skip(2).map(|t| t.id).collect();
    assert_eq!(removed_ids, vec![2, 3, 4, 5]);

    b.realloc_with_default(2).unwrap();
    assert_eq!(b.len(), 2);
    assert_slice_aligned(&b, 64);

    // Prefix preserved.
    for i in 0..2 {
        assert_eq!(b[i].id, i);
        assert_eq!(b[i].val, 2000 + i);
    }

    // Removed elements destroyed exactly once, and kept elements not dropped yet.
    assert_track_drop_counts(&[(0, 0), (1, 0)], &removed_ids);

    drop(b);
    assert_track_drop_exactly_once(next_track_id());
}

fn scenario_panic_during_shrink_then_safe_drop_no_double_drop() {
    reset_bomb(Some(5));

    let mut b = AlignedBox::<[BombTrack]>::slice_from_default(64, 8).unwrap();
    assert_eq!(b.len(), 8);
    assert_slice_aligned(&b, 64);

    for i in 0..8 {
        b[i].val = 10 + i;
        assert_eq!(b[i].id, i);
    }

    // Destructor panic during shrink is allowed to propagate.
    let r = catch_unwind(AssertUnwindSafe(|| {
        // If this returns Ok/Err without panicking, treat as unexpected in this scenario.
        match b.realloc_with_default(2) {
            Ok(()) => panic!("expected a destructor panic during shrink"),
            Err(_) => panic!("unexpected realloc error instead of destructor panic"),
        }
    }));
    assert!(r.is_err(), "expected unwind from destructor panic during shrink");

    // After the panic, dropping must not revisit destroyed elements (no double drops)
    // and must not panic a second time.
    let r2 = catch_unwind(AssertUnwindSafe(|| drop(b)));
    assert!(r2.is_ok(), "drop after shrink panic must not panic");

    assert_bomb_no_double_drop(bomb_next_id());
}

fn main() {
    scenario_scalar_new_alignment_upgrade_and_mutation();
    scenario_grow_preserve_prefix_and_default_init_suffix_and_alignment_and_no_leak();
    scenario_shrink_destroys_removed_exactly_once_and_preserves_prefix_and_alignment_and_no_leak();
    scenario_panic_during_shrink_then_safe_drop_no_double_drop();
}
