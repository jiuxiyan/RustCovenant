use rtrb::{PopError, PeekError, PushError, RingBuffer};
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

#[derive(Debug)]
struct Tracked {
    id: usize,
    drops_log: Rc<RefCell<Vec<usize>>>,
    drop_counts: Rc<RefCell<Vec<u32>>>,
    bomb: Rc<RefCell<Option<usize>>>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        {
            let mut counts = self.drop_counts.borrow_mut();
            counts[self.id] = counts[self.id].saturating_add(1);
        }
        self.drops_log.borrow_mut().push(self.id);

        let fire = *self.bomb.borrow() == Some(self.id);
        if fire {
            *self.bomb.borrow_mut() = None;
            panic!("injected destructor panic");
        }
    }
}

fn mk(
    id: usize,
    drops_log: &Rc<RefCell<Vec<usize>>>,
    drop_counts: &Rc<RefCell<Vec<u32>>>,
    bomb: &Rc<RefCell<Option<usize>>>,
) -> Tracked {
    Tracked {
        id,
        drops_log: drops_log.clone(),
        drop_counts: drop_counts.clone(),
        bomb: bomb.clone(),
    }
}

fn assert_counts_leq_one(counts: &Rc<RefCell<Vec<u32>>>, upto: usize) {
    let counts = counts.borrow();
    for (i, &c) in counts.iter().enumerate().take(upto + 1) {
        assert!(c <= 1, "drop count for id {i} is {c}, expected <= 1");
    }
}

fn scenario_normal_wrapped_partial_commit_fifo_and_capacity_return() {
    let drops_log = Rc::new(RefCell::new(Vec::<usize>::new()));
    let drop_counts = Rc::new(RefCell::new(vec![0u32; 64]));
    let bomb = Rc::new(RefCell::new(None));

    let (mut p, mut c) = RingBuffer::new(4);

    for id in 0..4 {
        p.push(mk(id, &drops_log, &drop_counts, &bomb)).unwrap();
    }

    // Create a wrapped layout: queue will later contain 2,3,4,5.
    let hold0 = c.pop().unwrap();
    let hold1 = c.pop().unwrap();
    for id in 4..6 {
        p.push(mk(id, &drops_log, &drop_counts, &bomb)).unwrap();
    }

    assert_eq!(c.slots(), 4);
    assert!(p.is_full());

    // Commit a prefix of a wrapped chunk.
    let chunk = c.read_chunk(4).unwrap();
    assert_eq!(chunk.len(), 4);
    chunk.commit(3);

    // Only the committed prefix is destroyed, in FIFO order.
    assert_eq!(&*drops_log.borrow(), &[2, 3, 4]);

    // The uncommitted tail remains readable.
    assert_eq!(c.slots(), 1);
    assert_eq!(c.peek().unwrap().id, 5);

    // Corresponding capacity is returned to the producer.
    assert_eq!(p.slots(), 3);
    for id in 6..=8 {
        assert_eq!(p.push(mk(id, &drops_log, &drop_counts, &bomb)), Ok(()));
    }

    // FIFO order across remaining + newly written items.
    let mut got = Vec::new();
    for _ in 0..4 {
        let v = c.pop().unwrap();
        got.push(v.id);
        drop(v);
    }
    assert_eq!(got, vec![5, 6, 7, 8]);
    assert_eq!(c.pop(), Err(PopError::Empty));

    // Now drop previously popped items.
    drop(hold0);
    drop(hold1);

    // Normal completion: no leaks, no double-drops.
    {
        let counts = drop_counts.borrow();
        for id in 0..=8 {
            assert_eq!(counts[id], 1, "id {id} should be dropped exactly once");
        }
    }
}

fn scenario_commit_zero_is_noop_and_preserves_elements() {
    let drops_log = Rc::new(RefCell::new(Vec::<usize>::new()));
    let drop_counts = Rc::new(RefCell::new(vec![0u32; 16]));
    let bomb = Rc::new(RefCell::new(None));

    let (mut p, mut c) = RingBuffer::new(2);
    p.push(mk(0, &drops_log, &drop_counts, &bomb)).unwrap();
    p.push(mk(1, &drops_log, &drop_counts, &bomb)).unwrap();

    assert!(p.is_full());
    let chunk = c.read_chunk(2).unwrap();
    assert_eq!(chunk.len(), 2);
    chunk.commit(0);

    // Zero commit must not drop or consume anything.
    assert!(drops_log.borrow().is_empty());
    assert_eq!(c.slots(), 2);
    assert!(p.is_full());

    let a = c.pop().unwrap();
    let b = c.pop().unwrap();
    assert_eq!(a.id, 0);
    assert_eq!(b.id, 1);
    drop(a);
    drop(b);

    {
        let counts = drop_counts.borrow();
        assert_eq!(counts[0], 1);
        assert_eq!(counts[1], 1);
    }
}

fn scenario_commit_all_and_read_chunk_zero() {
    let drops_log = Rc::new(RefCell::new(Vec::<usize>::new()));
    let drop_counts = Rc::new(RefCell::new(vec![0u32; 64]));
    let bomb = Rc::new(RefCell::new(None));

    let (mut p, mut c) = RingBuffer::new(3);

    // read_chunk(0) must be usable and commit_all must be a no-op.
    let z = c.read_chunk(0).unwrap();
    assert_eq!(z.len(), 0);
    z.commit_all();
    assert!(drops_log.borrow().is_empty());
    assert_eq!(p.slots(), 3);
    assert!(c.is_empty());

    for id in 10..=12 {
        p.push(mk(id, &drops_log, &drop_counts, &bomb)).unwrap();
    }

    let chunk = c.read_chunk(3).unwrap();
    chunk.commit_all();

    // FIFO destruction on normal completion.
    assert_eq!(&*drops_log.borrow(), &[10, 11, 12]);
    assert!(c.is_empty());
    assert_eq!(p.slots(), 3);

    {
        let counts = drop_counts.borrow();
        assert_eq!(counts[10], 1);
        assert_eq!(counts[11], 1);
        assert_eq!(counts[12], 1);
    }
}

fn scenario_overcommit_panics_without_consuming() {
    let (mut p, mut c) = RingBuffer::new(2);
    p.push(1i32).unwrap();
    p.push(2i32).unwrap();
    assert!(p.is_full());
    assert_eq!(c.slots(), 2);

    let r = catch_unwind(AssertUnwindSafe(|| {
        let chunk = c.read_chunk(2).unwrap();
        chunk.commit(3);
    }));
    assert!(r.is_err(), "overcommit must panic");

    // Must not consume elements on overcommit.
    assert_eq!(c.slots(), 2);
    assert!(p.is_full());
    assert_eq!(c.pop(), Ok(1));
    assert_eq!(c.pop(), Ok(2));
    assert_eq!(c.pop(), Err(PopError::Empty));
}

fn scenario_destructor_panic_during_commit_propagates_and_stays_safe() {
    let drops_log = Rc::new(RefCell::new(Vec::<usize>::new()));
    let drop_counts = Rc::new(RefCell::new(vec![0u32; 64]));
    let bomb = Rc::new(RefCell::new(None));

    let (mut p, mut c) = RingBuffer::new(4);

    for id in 0..4 {
        p.push(mk(id, &drops_log, &drop_counts, &bomb)).unwrap();
    }

    // Make the queue wrapped: remaining items in queue become 2,3,4,5.
    let hold0 = c.pop().unwrap();
    let hold1 = c.pop().unwrap();
    for id in 4..6 {
        p.push(mk(id, &drops_log, &drop_counts, &bomb)).unwrap();
    }

    // Cause a destructor panic on id 4, which is in the committed prefix.
    *bomb.borrow_mut() = Some(4);

    let r = catch_unwind(AssertUnwindSafe(|| {
        c.read_chunk(4).unwrap().commit(3);
    }));
    assert!(r.is_err(), "destructor panic should propagate out of commit");

    // The items whose Drop started must not be dropped twice.
    {
        let counts = drop_counts.borrow();
        assert_eq!(counts[2], 1);
        assert_eq!(counts[3], 1);
        assert_eq!(counts[4], 1);
    }

    // After catching the panic, safe operations must remain valid.
    // In particular, already-dropped ids must not be readable again.
    let mut seen = Vec::<usize>::new();
    for _ in 0..4 {
        match c.pop() {
            Ok(v) => {
                seen.push(v.id);
                drop(v);
            }
            Err(PopError::Empty) => break,
        }
    }
    assert!(!seen.contains(&2), "id 2 was dropped during commit but reappeared");
    assert!(!seen.contains(&3), "id 3 was dropped during commit but reappeared");
    assert!(!seen.contains(&4), "id 4 was dropped during commit but reappeared");

    // Dropping remaining handles must not trigger double-drops.
    drop(c);
    drop(p);
    drop(hold0);
    drop(hold1);

    assert_counts_leq_one(&drop_counts, 6);
}

fn scenario_capacity_zero_smoke() {
    let (mut p, mut c) = RingBuffer::<i32>::new(0);
    assert_eq!(p.slots(), 0);
    assert!(p.is_full());

    match p.push(1) {
        Err(PushError::Full(v)) => assert_eq!(v, 1),
        other => panic!("expected Full(1), got {other:?}"),
    }

    assert_eq!(c.pop(), Err(PopError::Empty));
    assert_eq!(c.peek(), Err(PeekError::Empty));

    // read_chunk(0) + commit_all must be usable even at capacity 0.
    c.read_chunk(0).unwrap().commit_all();
}

fn main() {
    scenario_normal_wrapped_partial_commit_fifo_and_capacity_return();
    scenario_commit_zero_is_noop_and_preserves_elements();
    scenario_commit_all_and_read_chunk_zero();
    scenario_overcommit_panics_without_consuming();
    scenario_destructor_panic_during_commit_propagates_and_stays_safe();
    scenario_capacity_zero_smoke();
}
