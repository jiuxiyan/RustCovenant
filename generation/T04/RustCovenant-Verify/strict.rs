use caja::Caja;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn snapshot_u8(c: &Caja<u8>) -> Vec<u8> {
    let mut v = Vec::with_capacity(c.len());
    for i in 0..c.len() {
        v.push(c[i]);
    }
    v
}

fn snapshot_u32(c: &Caja<u32>) -> Vec<u32> {
    let mut v = Vec::with_capacity(c.len());
    for i in 0..c.len() {
        v.push(c[i]);
    }
    v
}

fn assert_panics<F: FnOnce() -> R, R>(f: F) {
    let r = catch_unwind(AssertUnwindSafe(f));
    assert!(r.is_err(), "expected panic, but call completed");
}

fn main() {
    // Scenario 1: Caja::new with positive length and Copy numeric element; valid indices read/write.
    let mut a = Caja::<u8>::new(4, 7);
    assert_eq!(a.len(), 4);
    for i in 0..a.len() {
        assert_eq!(a[i], 7);
    }
    a[2] = 9;
    assert_eq!(a[2], 9);
    assert_eq!(a[0], 7);
    assert_eq!(a[1], 7);
    assert_eq!(a[3], 7);

    // Scenario 2: IndexMut must reject all indices >= len; failed access leaves contents and len unchanged.
    let before = snapshot_u8(&a);
    let before_len = a.len();
    assert_panics(|| {
        a[before_len] = 123;
    });
    assert_eq!(a.len(), before_len);
    assert_eq!(snapshot_u8(&a), before);

    // Scenario 3: Index must reject all indices >= len; failed read leaves contents and len unchanged.
    let before2 = snapshot_u8(&a);
    let before2_len = a.len();
    assert_panics(|| {
        let _x = a[before2_len];
    });
    assert_eq!(a.len(), before2_len);
    assert_eq!(snapshot_u8(&a), before2);

    // Scenario 4: Clone must allocate independent storage; mutations do not bleed across.
    let mut orig = Caja::<u32>::new(3, 42);
    orig[1] = 99;
    let mut cloned = orig.clone();

    assert_eq!(orig.len(), 3);
    assert_eq!(cloned.len(), 3);
    assert_eq!(snapshot_u32(&orig), snapshot_u32(&cloned));

    // Independence check: live allocations should not share the same pointer.
    assert_ne!(orig.as_mut_ptr(), cloned.as_mut_ptr(), "clone must not alias original buffer");

    cloned[1] = 1000;
    assert_eq!(orig[1], 99);
    assert_eq!(cloned[1], 1000);

    orig[0] = 1;
    assert_eq!(orig[0], 1);
    assert_eq!(cloned[0], 42);

    // Scenario 5: From<&[T]> copies data into a new heap buffer; later mutations don't affect source.
    let src: [u8; 5] = [1, 2, 3, 4, 5];
    let mut from = Caja::<u8>::from(&src[..]);
    assert_eq!(from.len(), src.len());
    for i in 0..src.len() {
        assert_eq!(from[i], src[i]);
    }
    from[0] = 9;
    assert_eq!(from[0], 9);
    assert_eq!(src[0], 1);

    // Additional OOB check on a fresh instance, ensuring no partial mutation happened.
    let mut b = Caja::<u8>::new(2, 7);
    b[0] = 8;
    let b_before = snapshot_u8(&b);
    assert_panics(|| {
        b[2] = 9;
    });
    assert_eq!(b.len(), 2);
    assert_eq!(snapshot_u8(&b), b_before);
}
