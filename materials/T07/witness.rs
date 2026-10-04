
use rtrb::RingBuffer;
use std::{cell::RefCell,rc::Rc,panic::{catch_unwind,AssertUnwindSafe}};
#[derive(Debug)]struct T{id:usize,heap:String,d:Rc<RefCell<Vec<usize>>>,bomb:Rc<RefCell<Option<usize>>>}
impl Drop for T{fn drop(&mut self){self.d.borrow_mut()[self.id]+=1;let fire=*self.bomb.borrow()==Some(self.id);if fire{*self.bomb.borrow_mut()=None;panic!("injected destructor panic");}}}
fn item(id:usize,d:&Rc<RefCell<Vec<usize>>>,b:&Rc<RefCell<Option<usize>>>)->T{T{id,heap:format!("item{id}"),d:d.clone(),bomb:b.clone()}}
fn fill(wrapped:bool,d:&Rc<RefCell<Vec<usize>>>,b:&Rc<RefCell<Option<usize>>>)->(rtrb::Producer<T>,rtrb::Consumer<T>){let(mut p,mut c)=RingBuffer::new(4);for id in 0..4{p.push(item(id,d,b)).unwrap();}if wrapped{drop(c.pop().unwrap());drop(c.pop().unwrap());for id in 4..6{p.push(item(id,d,b)).unwrap();}}(p,c)}
fn main(){let d=Rc::new(RefCell::new(vec![0;16]));let b=Rc::new(RefCell::new(None));let(p,mut c)=fill(true,&d,&b);*b.borrow_mut()=Some(4);let r=catch_unwind(AssertUnwindSafe(||c.read_chunk(4).unwrap().commit(3)));assert!(r.is_err());drop(c);drop(p);assert!(d.borrow().iter().all(|&x|x<=1));}
