
use aligned_box::AlignedBox;
use std::{cell::RefCell,panic::{catch_unwind,AssertUnwindSafe}};
thread_local!{static NEXT:RefCell<usize>=const{RefCell::new(0)};static DROPS:RefCell<Vec<usize>>=RefCell::new(vec![0;32]);static BOMB:RefCell<Option<usize>>=const{RefCell::new(None)};}
struct T{id:usize,heap:String}
impl Default for T{fn default()->Self{let id=NEXT.with(|x|{let id=*x.borrow();*x.borrow_mut()+=1;id});T{id,heap:format!("item{id}")}}}
impl Drop for T{fn drop(&mut self){DROPS.with(|x|x.borrow_mut()[self.id]+=1);let fire=BOMB.with(|x|{let fire=*x.borrow()==Some(self.id);if fire{*x.borrow_mut()=None;}fire});if fire{panic!("injected destructor panic");}}}
fn reset(bomb:Option<usize>){NEXT.with(|x|*x.borrow_mut()=0);DROPS.with(|x|*x.borrow_mut()=vec![0;32]);BOMB.with(|x|*x.borrow_mut()=bomb);}
fn check(b:&AlignedBox<[T]>,align:usize){assert_eq!((b.as_ptr() as usize)%align,0);let mut seen=vec![false;32];for x in b.iter(){assert_eq!(x.heap,format!("item{}",x.id));assert!(!seen[x.id]);seen[x.id]=true;DROPS.with(|d|assert_eq!(d.borrow()[x.id],0));}}
fn main(){for align in [16,64,128]{reset(None);let mut b=AlignedBox::<[T]>::slice_from_default(align,4).unwrap();b.realloc_with_default(7).unwrap();assert_eq!(b.len(),7);assert_eq!(b.iter().map(|x|x.id).collect::<Vec<_>>(),(0..7).collect::<Vec<_>>());check(&b,align);b.realloc_with_default(3).unwrap();assert_eq!(b.len(),3);assert_eq!(b.iter().map(|x|x.id).collect::<Vec<_>>(),vec![0,1,2]);check(&b,align);drop(b);DROPS.with(|d|for i in 0..7{assert_eq!(d.borrow()[i],1);});}}
