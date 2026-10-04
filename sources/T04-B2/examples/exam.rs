extern crate caja;
use caja::Caja;

#[allow(unused_mut)]
pub fn main() {
    // Initializes an array on the heap of boolean values
    // that are all set to true of size 6
    let bool_caja = Caja::<bool>::new(6, true).unwrap();
    print!("{}\n", bool_caja);

    
    // The initialized array has all it's values set to 0
    let int_caja = Caja::<u32>::new_zeroed(47).unwrap();
    print!("{}\n", int_caja);


    let mut some_size_that_changes = 88usize;
    // Cannot create a boxed value with a mutable as the size
    // let the_boxed_value = Box::<[f32;some_size_that_changes]>::new([0;some_size_that_changes]);

    // Caja doesn't give any problems.
    // Also the value is uninitialized, which is the fastest creation 
    // method for caja.
    let caja_value = Caja::<f32>::new_uninitialized(some_size_that_changes).unwrap();
    print!("{}\n", caja_value);
}
