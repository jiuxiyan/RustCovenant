
use rtrb::RingBuffer;
use std::{cell::RefCell,rc::Rc,panic::{catch_unwind,AssertUnwindSafe}};
#[derive(Debug)]struct T{id:usize,heap:String,d:Rc<RefCell<Vec<usize>>>,bomb:Rc<RefCell<Option<usize>>>}
impl Drop for T{fn drop(&mut self){self.d.borrow_mut()[self.id]+=1;let fire=*self.bomb.borrow()==Some(self.id);if fire{*self.bomb.borrow_mut()=None;panic!("injected destructor panic");}}}
fn item(id:usize,d:&Rc<RefCell<Vec<usize>>>,b:&Rc<RefCell<Option<usize>>>)->T{T{id,heap:format!("item{id}"),d:d.clone(),bomb:b.clone()}}
fn fill(wrapped:bool,d:&Rc<RefCell<Vec<usize>>>,b:&Rc<RefCell<Option<usize>>>)->(rtrb::Producer<T>,rtrb::Consumer<T>){let(mut p,mut c)=RingBuffer::new(4);for id in 0..4{p.push(item(id,d,b)).unwrap();}if wrapped{drop(c.pop().unwrap());drop(c.pop().unwrap());for id in 4..6{p.push(item(id,d,b)).unwrap();}}(p,c)}
fn main(){for wrapped in [false,true]{for n in 0..=4{let d=Rc::new(RefCell::new(vec![0;16]));let b=Rc::new(RefCell::new(None));let(mut p,mut c)=fill(wrapped,&d,&b);let start=if wrapped{2}else{0};c.read_chunk(4).unwrap().commit(n);assert_eq!(c.slots(),4-n);for id in 8..8+n{p.push(item(id,&d,&b)).unwrap();}let mut got=vec![];while let Ok(t)=c.pop(){assert_eq!(t.heap,format!("item{}",t.id));got.push(t.id);}assert_eq!(got,((start+n)..(start+4)).chain(8..8+n).collect::<Vec<_>>());drop(c);drop(p);for id in 0..start+4{assert_eq!(d.borrow()[id],1);}for id in 8..8+n{assert_eq!(d.borrow()[id],1);}}}}
