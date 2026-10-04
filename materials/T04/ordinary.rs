use caja::Caja;
use std::panic::{catch_unwind,AssertUnwindSafe};
fn main(){for n in 1usize..=8{let mut a=Caja::<u32>::new(n,42);assert_eq!(a.len(),n);assert_eq!(a.as_slice(),&vec![42;n][..]);for i in 0..n{a[i]=i as u32;}let mut b=a.clone();b[n-1]=999;assert_eq!(a[n-1],(n-1) as u32);for i in 0..n{assert_eq!(a[i],i as u32);}}}
