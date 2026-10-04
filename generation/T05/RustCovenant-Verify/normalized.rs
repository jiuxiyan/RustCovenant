use stack_collections::StackVec;
use std::{
    cell::RefCell,
    panic::{catch_unwind, AssertUnwindSafe},
    rc::Rc,
};

const CAP: usize = 8;

#[derive(Debug)]
struct Elem {
    id: usize,
    payload: String,
    drops: Rc<RefCell<Vec<usize>>>,
    bomb: Rc<RefCell<Option<usize>>>,
}

impl Drop for Elem {
    fn drop(&mut self) {
        self.drops.borrow_mut()[self.id] += 1;
        let fire = *self.bomb.borrow() == Some(self.id);
        if fire {
            *self.bomb.borrow_mut() = None;
            panic!("injected destructor panic");
        }
    }
}

fn make_vec(n: usize, drops: &Rc<RefCell<Vec<usize>>>, bomb: &Rc<RefCell<Option<usize>>>) -> StackVec<Elem, CAP> {
    assert!(n <= CAP);
    let mut v: StackVec<Elem, CAP> = StackVec::new();
    for id in 0..n {
        v.push(Elem {
            id,
            payload: format!("item{id}"),
            drops: drops.clone(),
            bomb: bomb.clone(),
        });
    }
    v
}

fn assert_unique_ids(v: &StackVec<Elem, CAP>) {
    let mut seen = vec![false; CAP];
    for e in v.as_slice() {
        assert!(e.id < CAP);
        assert!(!seen[e.id], "duplicate reachable id {}", e.id);
        seen[e.id] = true;
    }
}

fn assert_reachable_live(v: &StackVec<Elem, CAP>, drops: &Rc<RefCell<Vec<usize>>>, check_payload_item: bool) {
    assert_eq!(v.len(), v.as_slice().len());
    assert_eq!(v.capacity(), CAP);
    assert_unique_ids(v);
    for e in v.as_slice() {
        assert_eq!(drops.borrow()[e.id], 0, "destroyed element still reachable (id {})", e.id);
        if check_payload_item {
            assert_eq!(e.payload, format!("item{}", e.id));
        }
    }
}

fn scenario_normal_retain() {
    let drops = Rc::new(RefCell::new(vec![0usize; CAP]));
    let bomb = Rc::new(RefCell::new(None));

    let mut v = make_vec(6, &drops, &bomb);

    let call_order: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    v.retain({
        let call_order = call_order.clone();
        move |e: &mut Elem| {
            call_order.borrow_mut().push(e.id);
            if e.id % 2 == 0 {
                e.payload = format!("kept{}", e.id);
                true
            } else {
                false
            }
        }
    });

    assert_eq!(&*call_order.borrow(), &[0, 1, 2, 3, 4, 5]);
    assert_eq!(v.capacity(), CAP);
    assert_eq!(v.len(), 3);
    let ids: Vec<usize> = v.as_slice().iter().map(|e| e.id).collect();
    assert_eq!(&ids, &[0, 2, 4]);
    for e in v.as_slice() {
        assert_eq!(e.payload, format!("kept{}", e.id));
        assert_eq!(drops.borrow()[e.id], 0);
    }
    assert_eq!(drops.borrow()[1], 1);
    assert_eq!(drops.borrow()[3], 1);
    assert_eq!(drops.borrow()[5], 1);

    drop(v);
    // On normal completion, everything that existed should be dropped exactly once.
    for id in 0..6 {
        assert_eq!(drops.borrow()[id], 1, "id {} not dropped exactly once on normal path", id);
    }
    for id in 6..CAP {
        assert_eq!(drops.borrow()[id], 0);
    }
}

fn scenario_predicate_panic_invariants() {
    let drops = Rc::new(RefCell::new(vec![0usize; CAP]));
    let bomb = Rc::new(RefCell::new(None));

    let mut v = make_vec(5, &drops, &bomb);
    let visited: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));

    let r = catch_unwind(AssertUnwindSafe({
        let visited = visited.clone();
        move || {
            v.retain(move |e: &mut Elem| {
                visited.borrow_mut().push(e.id);
                if e.id == 2 {
                    panic!("injected predicate panic");
                }
                e.id != 1
            });
        }
    }));
    assert!(r.is_err());
    assert_eq!(&visited.borrow()[..], &[0, 1, 2]);

    // After catch_unwind, reachable elements must still be live/owned once and safely accessible.
    assert_reachable_live(&v, &drops, true);
    assert!(drops.borrow().iter().all(|&c| c <= 1), "double-drop observed after predicate panic");

    // Further safe operations must remain valid.
    let before_ids: Vec<usize> = v.as_slice().iter().map(|e| e.id).collect();
    let before_counts = drops.borrow().clone();

    v.retain(|_| true);

    let after_ids: Vec<usize> = v.as_slice().iter().map(|e| e.id).collect();
    assert_eq!(after_ids, before_ids);
    assert_eq!(drops.borrow().as_slice(), before_counts.as_slice());

    let ids_at_drop: Vec<usize> = v.as_slice().iter().map(|e| e.id).collect();
    drop(v);

    for id in ids_at_drop {
        assert_eq!(drops.borrow()[id], 1, "reachable id {} not dropped when vector dropped", id);
    }
    assert!(drops.borrow().iter().all(|&c| c <= 1), "double-drop observed after dropping vector");
}

fn scenario_destructor_panic_in_retain() {
    let drops = Rc::new(RefCell::new(vec![0usize; CAP]));
    let bomb = Rc::new(RefCell::new(Some(2usize)));

    let mut v = make_vec(4, &drops, &bomb);

    let r = catch_unwind(AssertUnwindSafe(|| {
        v.retain(|e: &mut Elem| e.id != 2);
    }));

    // Destructor panic may propagate; accept either outcome.
    let _ = r;

    // Regardless, vector must remain safely accessible and internally consistent.
    assert_reachable_live(&v, &drops, true);
    assert!(drops.borrow().iter().all(|&c| c <= 1), "double-drop observed around destructor panic");

    // Dropping later must remain memory-safe; panic is allowed, so catch it.
    let r2 = catch_unwind(AssertUnwindSafe(|| drop(v)));
    let _ = r2;
    assert!(drops.borrow().iter().all(|&c| c <= 1), "double-drop observed after final drop");
}

fn main() {
    scenario_normal_retain();
    scenario_predicate_panic_invariants();
    scenario_destructor_panic_in_retain();
}
