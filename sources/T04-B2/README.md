# Caja

---

Caja is a simple rust library that allows for the creation of fixed sized arrays of a size unknown at compile time. It is basically `Box<[T;n]>` but allowing the `n` to be non constant value.

## Example

~~~ rust

extern crate caja;

use caja::Caja;



pub fn main() {

    // Creates a heap allocated array of size 108 with the default value 0xEE

    let caj = Caja::<u16>::new(108, 0xEE).unwrap();

    // it is also possible to use new_zeroed and new_uninitialized



    // Caja implements Display and Debug, as long as T does so too.

    println!("{}", caj);

    

    // Caja implements Index and IndexMut, so it is possible to access it as any normal array.

    println!("{}", caj[77]);



    //  And you can also access the underlying pointer inside of Caja

    println!("{:p}", caj.as_mut_ptr());

}

~~~












