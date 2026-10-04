use rtrb::{PopError, PushError, RingBuffer};
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

fn assert_pop_err_empty<T>(c: &mut rtrb::Consumer<T>) {
    assert!(matches!(c.pop(), Err(PopError::Empty)));
}

fn scenario_drop_chunk_without_commit_preserves() {
    let (mut p, mut c) = RingBuffer::new(2);
    p.push(1).unwrap();
    p.push(2).unwrap();

    let chunk = c.read_chunk(2).unwrap();
    let (a, b) = chunk.as_slices();
    assert_eq!(a.len() + b.len(), 2);
    assert_eq!(a, &[1, 2]);
    assert!(b.is_empty());

    // Dropping ReadChunk without commit must not consume or drop items.
    drop(chunk);
    assert_eq!(c.peek().copied(), Ok(1));
    assert_eq!(c.pop(), Ok(1));
    assert_eq!(c.pop(), Ok(2));
    assert_pop_err_empty(&mut c);
}

fn scenario_commit_zero_consumes_nothing() {
    let (mut p, mut c) = RingBuffer::new(3);
    for v in [10, 11, 12] {
        p.push(v).unwrap();
    }

    let chunk = c.read_chunk(3).unwrap();
    let (a, b) = chunk.as_slices();
    assert_eq!(a, &[10, 11, 12]);
    assert!(b.is_empty());

    // Commiting 0 should keep everything readable and not free space.
    chunk.commit(0);
    assert_eq!(p.slots(), 0);
    assert_eq!(c.slots(), 3);
    assert_eq!(c.peek().copied(), Ok(10));

    assert_eq!(c.pop(), Ok(10));
    assert_eq!(c.pop(), Ok(11));
    assert_eq!(c.pop(), Ok(12));
    assert_pop_err_empty(&mut c);
    assert_eq!(p.slots(), 3);
}

fn scenario_partial_commit_frees_space_and_preserves_suffix() {
    let (mut p, mut c) = RingBuffer::new(4);
    for v in [1, 2, 3, 4] {
        p.push(v).unwrap();
    }

    let chunk = c.read_chunk(4).unwrap();
    let (a, b) = chunk.as_slices();
    assert_eq!(a, &[1, 2, 3, 4]);
    assert!(b.is_empty());

    chunk.commit(2);
    // Two items were committed/dropped and space returned to producer.
    assert_eq!(p.slots(), 2);
    assert_eq!(c.slots(), 2);

    // Remaining items must still be readable in FIFO order.
    assert_eq!(c.pop(), Ok(3));
    assert_eq!(c.pop(), Ok(4));
    assert_pop_err_empty(&mut c);

    // Those two freed slots should accept new pushes.
    assert_eq!(p.push(5), Ok(()));
    assert_eq!(p.push(6), Ok(()));
    assert!(matches!(p.push(7), Err(PushError::Full(7))));

    assert_eq!(c.pop(), Ok(5));
    assert_eq!(c.pop(), Ok(6));
    assert_pop_err_empty(&mut c);
}

fn scenario_wrapped_chunk_slices_and_commit_all() {
    let (mut p, mut c) = RingBuffer::new(4);

    for v in [0, 1, 2, 3] {
        p.push(v).unwrap();
    }
    assert_eq!(c.pop(), Ok(0));
    assert_eq!(c.pop(), Ok(1));

    // Wrap the tail by pushing two more.
    p.push(4).unwrap();
    p.push(5).unwrap();

    let chunk = c.read_chunk(4).unwrap();
    let (a, b) = chunk.as_slices();
    // Expect a wrapped view: first slice from current head to end, second from start.
    assert_eq!(a, &[2, 3]);
    assert_eq!(b, &[4, 5]);

    chunk.commit_all();
    assert_pop_err_empty(&mut c);
    assert_eq!(p.slots(), 4);
}

fn scenario_overcommit_panics_and_does_not_consume() {
    let (mut p, mut c) = RingBuffer::new(2);
    p.push(10).unwrap();
    p.push(11).unwrap();

    let r = catch_unwind(AssertUnwindSafe(|| {
        let chunk = c.read_chunk(2).unwrap();
        chunk.commit(3); // must panic
    }));
    assert!(r.is_err());

    // Overcommit must not consume any elements.
    assert_eq!(c.pop(), Ok(10));
    assert_eq!(c.pop(), Ok(11));
    assert_pop_err_empty(&mut c);
    assert_eq!(p.slots(), 2);
}

#[derive(Debug)]
struct Bomb {
    id: usize,
    drops: Rc<RefCell<Vec<usize>>>,
    panic_on: Rc<RefCell<Option<usize>>>,
}

impl Drop for Bomb {
    fn drop(&mut self) {
        self.drops.borrow_mut()[self.id] += 1;
        if *self.panic_on.borrow() == Some(self.id) {
            *self.panic_on.borrow_mut() = None;
            panic!("injected destructor panic");
        }
    }
}

fn scenario_destructor_panic_during_commit_keeps_queue_usable() {
    let drops = Rc::new(RefCell::new(vec![0usize; 8]));
    let panic_on = Rc::new(RefCell::new(Some(1usize)));

    let (mut p, mut c) = RingBuffer::new(4);
    for id in 0..4 {
        p.push(Bomb {
            id,
            drops: drops.clone(),
            panic_on: panic_on.clone(),
        })
        .unwrap();
    }

    // Commiting a prefix should drop in FIFO order; we inject a panic on id=1.
    let r = catch_unwind(AssertUnwindSafe(|| {
        let chunk = c.read_chunk(4).unwrap();
        chunk.commit(3);
    }));
    assert!(r.is_err());

    // The first two drops (0 then 1) must have happened exactly once.
    // Items after the panic point must not have been dropped yet.
    let d = drops.borrow();
    assert_eq!(d[0], 1);
    assert_eq!(d[1], 1);
    assert_eq!(d[2], 0);
    assert_eq!(d[3], 0);
    drop(d);

    // After catching, safe operations must remain valid.
    // Destroyed slots must not remain readable.
    // The next readable element should be id=2 (if the implementation advanced past dropped ones).
    let next = c.pop().unwrap();
    assert_eq!(next.id, 2);
    drop(next);
    let next2 = c.pop().unwrap();
    assert_eq!(next2.id, 3);
    drop(next2);
    assert_pop_err_empty(&mut c);

    // No element should ever be dropped twice.
    drop(c);
    drop(p);
    assert!(drops.borrow().iter().all(|&n| n <= 1));
}

fn main() {
    scenario_drop_chunk_without_commit_preserves();
    scenario_commit_zero_consumes_nothing();
    scenario_partial_commit_frees_space_and_preserves_suffix();
    scenario_wrapped_chunk_slices_and_commit_all();
    scenario_overcommit_panics_and_does_not_consume();
    scenario_destructor_panic_during_commit_keeps_queue_usable();
}
