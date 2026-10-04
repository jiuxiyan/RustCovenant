extern crate caja;
use caja::Caja;

#[allow(unused_mut)]
pub fn main() {
    // Initializes an array on the heap of boolean values
    // that are all set to true of size 6
    let bool_caja = Caja::<bool>::new(6, true);
    print!("{}\n", bool_caja);

    
    // The initialized array has all it's values set to 0
    // (unsafe because Caja doesn't know if the element type
    // can be safely set to zero like an integer, or not
    // like a reference)
    let int_caja = unsafe { Caja::<u32>::new_zeroed(47) };
    print!("{:?}\n", int_caja); // Implements debug

    // We can access any random element with thearray syntax
    assert_eq!(int_caja[33], 0);

    // Caja will complain if the index is greater than the size:
    // int_caja[90] -> panics because 90 >= 47
    // To prevent this, one could use unchecked_index, however,
    // that will certantly cause UB
    // let ub = unsafe { int_caja.unchecked_index(90) }; -> please don't

    let mut some_size_that_changes = 88usize;
    // Cannot create a boxed value with a mutable as the size
    // let the_boxed_value = Box::<[f32;some_size_that_changes]>::new([0;some_size_that_changes]);

    // Caja doesn't give any problems.
    // Also the value may be uninitialized, which is the fastest creation 
    // method for caja.
    let mut float_caja = unsafe { Caja::<f32>::new_uninitialized(some_size_that_changes) };
    print!("{}\n", float_caja);

    // Of course, only having unitialized data is pretty useless, so you can
    // iterate through the Caja with a for loop
    let mut angle: f32 = 0.0;
    for i in &mut float_caja {
        *i = angle.sin();
        angle += 0.1;
    }

    // And we can also iterate without it being mut
    for i in &float_caja {
        println!("{i}");
    }

    // Or we could treat ut as a slice
    let slice: &[f32] = float_caja.as_slice();
    _ = slice;

    // Or even as a pointer
    let ptr: *mut f32 = float_caja.as_mut_ptr();
    _ = ptr;

    // And at last, here the drops are called
}
