use stack_collections::{StackArrayString, StackString, StackVec};
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

#[derive(Debug)]
struct Item {
    id: usize,
    payload: String,
    drops: Rc<RefCell<Vec<usize>>>,
    bomb: Rc<RefCell<Option<usize>>>,
}

impl Item {
    fn new(id: usize, drops: &Rc<RefCell<Vec<usize>>>, bomb: &Rc<RefCell<Option<usize>>>) -> Self {
        Self {
            id,
            payload: format!("item{id}"),
            drops: drops.clone(),
            bomb: bomb.clone(),
        }
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        self.drops.borrow_mut()[self.id] += 1;
        let should_panic = *self.bomb.borrow() == Some(self.id);
        if should_panic {
            *self.bomb.borrow_mut() = None;
            panic!("injected destructor panic");
        }
    }
}

fn make_vec(
    n: usize,
    drops: &Rc<RefCell<Vec<usize>>>,
    bomb: &Rc<RefCell<Option<usize>>>,
) -> StackVec<Item, 8> {
    let mut v: StackVec<Item, 8> = StackVec::new();
    for id in 0..n {
        v.push(Item::new(id, drops, bomb));
    }
    v
}

fn assert_live_unique(v: &StackVec<Item, 8>, drops: &Rc<RefCell<Vec<usize>>>) {
    assert_eq!(v.len(), v.as_slice().len());
    assert_eq!(v.capacity(), 8);

    let mut seen = [false; 8];
    for it in v.as_slice() {
        assert!(it.id < 8);
        assert!(!seen[it.id], "duplicate reachable id {}", it.id);
        seen[it.id] = true;
        assert_eq!(
            drops.borrow()[it.id],
            0,
            "dropped element still reachable: id {}",
            it.id
        );
        assert_eq!(it.payload, format!("item{}", it.id));
    }
}

fn assert_drop_counts_at_most_once(drops: &Rc<RefCell<Vec<usize>>>, max_id: usize) {
    for id in 0..max_id {
        let c = drops.borrow()[id];
        assert!(c <= 1, "id {} dropped more than once: {}", id, c);
    }
}

fn scenario_retain_normal_semantics() {
    let drops = Rc::new(RefCell::new(vec![0usize; 8]));
    let bomb = Rc::new(RefCell::new(None));

    let mut v = make_vec(5, &drops, &bomb);

    let visited = Rc::new(RefCell::new(Vec::<usize>::new()));
    v.retain(|it| {
        visited.borrow_mut().push(it.id);

        // Mutate some elements via the predicate; mutations to kept elements must persist.
        if it.id == 2 {
            it.payload.push_str("_kept");
        }
        if it.id == 3 {
            it.payload.push_str("_removed");
        }

        // Keep evens only.
        it.id % 2 == 0
    });

    assert_eq!(&*visited.borrow(), &[0, 1, 2, 3, 4]);
    assert_eq!(v.len(), 3);
    assert_eq!(
        v.as_slice().iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![0, 2, 4]
    );
    assert_eq!(v.capacity(), 8);
    assert_eq!(v.remaining_capacity(), 5);

    // Removed elements (1,3) must be dropped exactly once on normal completion.
    assert_eq!(drops.borrow()[1], 1);
    assert_eq!(drops.borrow()[3], 1);
    // Kept elements must still be live.
    assert_eq!(drops.borrow()[0], 0);
    assert_eq!(drops.borrow()[2], 0);
    assert_eq!(drops.borrow()[4], 0);

    // Predicate mutations must be preserved for retained elements.
    assert_eq!(v.as_slice()[1].id, 2);
    assert_eq!(v.as_slice()[1].payload, "item2_kept");
    assert_eq!(v.as_slice()[0].payload, "item0");
    assert_eq!(v.as_slice()[2].payload, "item4");

    drop(v);
    // After dropping the vector, all originally created elements should be dropped once.
    for id in 0..5 {
        assert_eq!(drops.borrow()[id], 1, "id {} not dropped exactly once", id);
    }
}

fn scenario_retain_predicate_panic_safety() {
    let drops = Rc::new(RefCell::new(vec![0usize; 8]));
    let bomb = Rc::new(RefCell::new(None));
    let mut v = make_vec(5, &drops, &bomb);

    let r = catch_unwind(AssertUnwindSafe(|| {
        v.retain(|it| {
            if it.id == 2 {
                panic!("predicate panic");
            }
            it.id != 1
        });
    }));
    assert!(r.is_err());

    // After unwinding, the vector must still be safely usable (len matches slice, access ok).
    assert_eq!(v.len(), v.as_slice().len());
    assert_eq!(v.capacity(), 8);
    assert!(v.len() <= 5);

    // All *reachable* elements must still be live, unique, and valid to inspect.
    assert_live_unique(&v, &drops);

    // Further safe operations should remain valid.
    v.retain(|_| true);
    assert_live_unique(&v, &drops);

    drop(v);
    // No element should be dropped more than once (leaks allowed on panic).
    assert_drop_counts_at_most_once(&drops, 5);
}

fn scenario_retain_drop_panic_safety() {
    let drops = Rc::new(RefCell::new(vec![0usize; 8]));
    let bomb = Rc::new(RefCell::new(Some(1usize)));
    let mut v = make_vec(4, &drops, &bomb);

    let r = catch_unwind(AssertUnwindSafe(|| {
        // Removing id=1 should trigger a destructor panic once.
        v.retain(|it| it.id != 1);
    }));
    assert!(r.is_err(), "expected destructor panic to propagate");

    // The drop-bomb should have been cleared by the panicking Drop impl.
    assert!(bomb.borrow().is_none());

    // The vector must still be safe to access and later drop.
    assert_eq!(v.len(), v.as_slice().len());
    assert_eq!(v.capacity(), 8);
    for it in v.as_slice() {
        assert!(it.id < 8);
        assert_eq!(
            drops.borrow()[it.id],
            0,
            "reachable element was already dropped"
        );
        assert_eq!(it.payload, format!("item{}", it.id));
    }

    // Dropping should not panic again.
    let r2 = catch_unwind(AssertUnwindSafe(|| drop(v)));
    assert!(r2.is_ok(), "drop after destructor panic must remain safe");

    assert_drop_counts_at_most_once(&drops, 4);
}

fn scenario_stackstring_and_alias() {
    // StackString boundary + try_* behavior.
    let mut s: StackString<10> = StackString::new();
    assert!(s.is_empty());
    assert_eq!(s.capacity(), 10);
    s.push_str("hi");
    s.push('😀'); // 4 bytes
    assert_eq!(s.as_str(), "hi😀");
    assert_eq!(s.len(), "hi😀".len());
    assert_eq!(s.remaining_capacity(), 10 - s.len());

    assert!(s.try_push_str("!!!!").is_some());
    assert!(s.is_full());
    let before = s.as_str().to_string();
    assert!(s.try_push('x').is_none());
    assert_eq!(s.as_str(), before);

    assert_eq!(s.pop(), '!');
    assert!(!s.is_full());

    // truncate must panic if not on a UTF-8 char boundary and must not corrupt the string.
    let mut s2: StackString<16> = StackString::new();
    s2.push_str("hello😀");
    let r = catch_unwind(AssertUnwindSafe(|| {
        s2.truncate(6); // splits the 4-byte emoji
    }));
    assert!(r.is_err());
    assert_eq!(s2.as_str(), "hello😀");

    // StackArrayString alias smoke test.
    let mut arr: StackArrayString<8, 3> = StackVec::new();
    arr.push(StackString::<8>::try_from("a").unwrap());
    arr.push(StackString::<8>::try_from("bb").unwrap());
    arr.push(StackString::<8>::try_from("ccc").unwrap());
    assert_eq!(arr.len(), 3);
    assert!(arr.is_full());
    assert_eq!(arr.capacity(), 3);
    assert_eq!(arr[0].as_str(), "a");
    assert_eq!(arr[1].as_str(), "bb");
    assert_eq!(arr[2].as_str(), "ccc");
    assert!(arr.try_push(StackString::<8>::try_from("dddd").unwrap()).is_none());

    // Mutating through the vector should work.
    arr.index_mut(1).push('!');
    assert_eq!(arr[1].as_str(), "bb!");
}

fn main() {
    scenario_retain_normal_semantics();
    scenario_retain_predicate_panic_safety();
    scenario_retain_drop_panic_safety();
    scenario_stackstring_and_alias();
}
