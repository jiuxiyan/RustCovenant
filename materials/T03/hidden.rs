use stable_vec::StableVec;
use std::{cell::Cell,rc::Rc,panic::{catch_unwind,AssertUnwindSafe}};
struct Item {id:usize,drops:Rc<Vec<Cell<usize>>>,bomb:bool}
impl Drop for Item {fn drop(&mut self){
 let c=&self.drops[self.id];c.set(c.get()+1);assert!(c.get()<=1,"duplicate destruction");
 if self.bomb {panic!("injected destructor panic")}
}}
fn main(){
 for bomb in [None,Some(0),Some(2),Some(4)] {for hole in [None,Some(1),Some(3)] {
  let d=Rc::new((0..6).map(|_|Cell::new(0)).collect::<Vec<_>>());
  let mut v=StableVec::new();for i in 0..5{assert_eq!(v.push(Item{id:i,drops:d.clone(),bomb:bomb==Some(i)}),i);}
  if let Some(h)=hole{drop(v.remove(h));}
  let cap=v.capacity();let result=catch_unwind(AssertUnwindSafe(||v.clear()));
  if bomb.is_none(){assert!(result.is_ok());assert_eq!(v.num_elements(),0);assert_eq!(v.next_push_index(),0);assert_eq!(v.capacity(),cap);}
  else {assert!(result.is_err());}
  let occupied=(0..v.capacity()).filter(|&i|v.has_element_at(i)).collect::<Vec<_>>();
  assert_eq!(occupied.len(),v.num_elements(),"occupancy count disagrees");
  for &idx in &occupied {let x=v.get(idx).unwrap();assert_eq!(x.id,idx);assert_eq!(d[x.id].get(),0,"dropped slot is reachable");}
  assert_eq!(v.iter().map(|(i,_)|i).collect::<Vec<_>>(),occupied);
  drop(v);assert!(d.iter().all(|c|c.get()<=1));
  if bomb.is_none(){assert!(d.iter().take(5).all(|c|c.get()==1));}
 }}
 let mut v=StableVec::from(&[8u32,9,10]);v.clear();assert_eq!(v.push(22),0);assert_eq!(v.get(0),Some(&22));
}
