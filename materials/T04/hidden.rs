use caja::Caja;
use std::panic::{catch_unwind,AssertUnwindSafe};
fn main(){for n in 1usize..=8{for bad in [n,n+1,usize::MAX]{let mut a=Caja::<i64>::new(n,-3);a[n-1]=123;let expected=a.as_slice().to_vec();let r=catch_unwind(AssertUnwindSafe(||{std::hint::black_box(a[bad]);}));assert!(r.is_err());assert_eq!(a.as_slice(),expected);let r=catch_unwind(AssertUnwindSafe(||{a[bad]=10;}));assert!(r.is_err());assert_eq!(a.len(),n);assert_eq!(a.as_slice(),expected);}}}
