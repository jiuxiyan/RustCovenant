
use stack_collections::StackVec;
use std::{cell::RefCell,rc::Rc,panic::{catch_unwind,AssertUnwindSafe}};
struct T{id:usize,heap:String,d:Rc<RefCell<Vec<usize>>>,bomb:Rc<RefCell<Option<usize>>>}
impl Drop for T{fn drop(&mut self){self.d.borrow_mut()[self.id]+=1;let fire=*self.bomb.borrow()==Some(self.id);if fire{*self.bomb.borrow_mut()=None;panic!("injected destructor panic");}}}
fn make(n:usize,d:&Rc<RefCell<Vec<usize>>>,b:&Rc<RefCell<Option<usize>>>)->StackVec<T,8>{let mut v=StackVec::new();for id in 0..n{v.push(T{id,heap:format!("item{id}"),d:d.clone(),bomb:b.clone()});}v}
fn check(v:&StackVec<T,8>,d:&Rc<RefCell<Vec<usize>>>){let mut seen=vec![false;8];for x in v.as_slice(){assert_eq!(x.heap,format!("item{}",x.id));assert!(!seen[x.id]);seen[x.id]=true;assert_eq!(d.borrow()[x.id],0,"destroyed element still reachable");}assert_eq!(v.len(),v.as_slice().len());assert_eq!(v.capacity(),8);}
fn main(){for n in 0..=6{for mode in 0..3{let d=Rc::new(RefCell::new(vec![0;8]));let b=Rc::new(RefCell::new(None));let mut v=make(n,&d,&b);let mut calls=vec![];v.retain(|x|{calls.push(x.id);x.heap.push('!');match mode{0=>true,1=>false,_=>x.id%2==0}});assert_eq!(calls,(0..n).collect::<Vec<_>>());let expected=(0..n).filter(|id|mode==0 || (mode==2&&id%2==0)).collect::<Vec<_>>();assert_eq!(v.iter().map(|x|x.id).collect::<Vec<_>>(),expected);for x in v.iter(){assert_eq!(x.heap,format!("item{}!",x.id));}assert_eq!(v.capacity(),8);drop(v);for id in 0..n{assert_eq!(d.borrow()[id],1);}}}}
