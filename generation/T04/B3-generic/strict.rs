// Standalone regression test for caja crate (edition 2021).
// Exercises only safe public API.

extern crate caja;

use caja::Caja;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn assert_panics<F: FnOnce()>(f: F) {
    let r = catch_unwind(AssertUnwindSafe(f));
    assert!(r.is_err(), "expected panic, but closure completed successfully");
}

fn snapshot_u8_4(a: &Caja<u8>) -> [u8; 4] {
    [a[0], a[1], a[2], a[3]]
}

fn main() {
    // Scenario 1: basic construction, len, in-bounds read/write via Index/IndexMut.
    let mut a = Caja::<u8>::new(4, 7);
    assert_eq!(a.len(), 4);
    for i in 0..a.len() {
        assert_eq!(a[i], 7, "new() must fill with default value");
    }
    a[0] = 1;
    a[1] = 2;
    a[2] = 3;
    a[3] = 4;
    assert_eq!(snapshot_u8_4(&a), [1, 2, 3, 4]);

    // Scenario 2: bounds checking for Index/IndexMut (must reject i >= len before access).
    let before = snapshot_u8_4(&a);
    let before_len = a.len();

    assert_panics(|| {
        let _ = a[4];
    });
    assert_panics(|| {
        a[4] = 9;
    });
    assert_panics(|| {
        a[usize::MAX] = 0;
    });

    // Failed access must leave initialized contents and length unchanged.
    assert_eq!(a.len(), before_len);
    assert_eq!(snapshot_u8_4(&a), before);

    // Still usable after a caught bounds panic.
    a[2] = 42;
    assert_eq!(a[2], 42);

    // Scenario 3: clone independence (deep copy of data).
    let mut b = a.clone();
    assert_eq!(b.len(), a.len());
    assert_eq!(snapshot_u8_4(&b), snapshot_u8_4(&a));

    b[0] = 99;
    assert_eq!(b[0], 99);
    assert_eq!(a[0], before[0], "mutating clone must not affect original");

    a[1] = 77;
    assert_eq!(a[1], 77);
    assert_ne!(b[1], a[1], "mutating original must not affect clone");

    // Scenario 4: From<&[T]> copies data (and does not alias the input slice).
    let src: [u16; 3] = [10, 20, 30];
    let mut c: Caja<u16> = Caja::from(&src[..]);
    assert_eq!(c.len(), 3);
    assert_eq!(c[0], 10);
    assert_eq!(c[1], 20);
    assert_eq!(c[2], 30);

    c[0] = 500;
    assert_eq!(c[0], 500);
    assert_eq!(src[0], 10, "Caja::from must copy, not alias the source slice");
}
