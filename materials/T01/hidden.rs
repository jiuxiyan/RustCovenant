use toodee::{TooDee,TooDeeOps};
use std::{cell::Cell,rc::Rc,panic::{catch_unwind,AssertUnwindSafe}};
struct Item { id: usize, drops: Rc<Vec<Cell<usize>>> }
impl Drop for Item { fn drop(&mut self) {
 let c=&self.drops[self.id]; c.set(c.get()+1); assert!(c.get()<=1,"duplicate destruction");
}}
fn item(id:usize,d:&Rc<Vec<Cell<usize>>>)->Item { Item{id,drops:d.clone()} }
struct Faulty { values:std::vec::IntoIter<Item>, declared:usize, panic_at:Option<usize>, calls:usize }
impl Iterator for Faulty { type Item=Item; fn next(&mut self)->Option<Item>{
 let k=self.calls; self.calls+=1; if self.panic_at==Some(k){panic!("injected iterator panic")};self.values.next()
}}
impl ExactSizeIterator for Faulty {fn len(&self)->usize{self.declared}}
fn main(){
 for cols in 1..=3 { for rows in 1..=3 { for pos in 0..=rows {
  let total=cols*(rows+1);let d=Rc::new((0..total).map(|_|Cell::new(0)).collect::<Vec<_>>());
  let mut g=TooDee::from_vec(cols,rows,(0..cols*rows).map(|i|item(i,&d)).collect());
  g.insert_row(pos,(cols*rows..total).map(|i|item(i,&d)).collect::<Vec<_>>());
  let expected=(0..pos*cols).chain(cols*rows..total).chain(pos*cols..rows*cols).collect::<Vec<_>>();
  assert_eq!(g.data().iter().map(|x|x.id).collect::<Vec<_>>(),expected);
  assert_eq!(g.size(),(cols,rows+1));drop(g);assert!(d.iter().all(|c|c.get()==1));
 }}}
 for pos in 0..=2 {for panic in [false,true] {
  let d=Rc::new((0..5).map(|_|Cell::new(0)).collect::<Vec<_>>());
  let mut g=TooDee::from_vec(2,2,(0..4).map(|i|item(i,&d)).collect());
  let it=Faulty{values:vec![item(4,&d)].into_iter(),declared:2,panic_at:if panic{Some(1)}else{None},calls:0};
  let result=catch_unwind(AssertUnwindSafe(||g.insert_row(pos,it)));
  drop(g);assert!(result.is_err());assert!(d.iter().all(|c|c.get()<=1));
 }}
}
