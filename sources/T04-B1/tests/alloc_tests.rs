#![cfg(test)]
#![allow(dead_code)]
#![allow(unused_assignments)]

extern crate caja;

#[test]
fn allocating() {
    let mut c = caja::Caja::<u8>::new_uninitialized(0xFF).unwrap();
    for i in 0..0xFF {
        c[i as usize] = i;
        assert_eq!(c[i as usize], i);
    }

    let d = caja::Caja::<u8>::new_zeroed(0xFF).unwrap();
    for i in 0..0xFF {
        assert_eq!(d[i], 0);
    }

    let e = caja::Caja::<u8>::new(0xFF, 0xAE).unwrap();
    for i in 0..0xFF {
        assert_eq!(e[i], 0xAE);
    }
}

#[test]
fn from_slice() {
    let arr = [8u8;743];
    let caj = caja::Caja::<u8>::try_from(&arr[..]).unwrap();

    assert_eq!(caj.len(), arr.len());
    for i in 0..743 {
        assert_eq!(caj[i], arr[i]);
    }
}

#[test]
fn display() {
    let mut caj = caja::Caja::<u16>::new_uninitialized(564).unwrap();
    for i in 0..564u16 {
        caj[i as usize] = i;
    }

    // Because of this, this test requires --nocapture flag
    println!("{}\n", caj);

    println!("{:?}", caj)
}
