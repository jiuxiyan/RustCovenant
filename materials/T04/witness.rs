use caja::Caja;
use std::panic::{catch_unwind,AssertUnwindSafe};
fn main(){let mut a=Caja::<u8>::new(2,7);let r=catch_unwind(AssertUnwindSafe(||{a[2]=9;}));assert!(r.is_err());}
