use vagus::essential::collections::StableVec;

#[test]
fn small_stable_vec_accepts_pushes_within_capacity() {
    let mut values = StableVec::with_capacity(2).expect("reserve two elements");

    values.push(10_u32).unwrap();
    values.push(20_u32).unwrap();

    assert_eq!(values.as_slice(), &[10, 20]);
    assert!(values.push(30).is_err());
    assert!(values.insert(1, 30).is_err());
    assert_eq!(values.as_slice(), &[10, 20]);
}

#[test]
fn stable_vec_handles_front_removal_insertion_and_append_in_order() {
    let mut values = StableVec::with_capacity(5).unwrap();
    values.extend([1, 2, 3]);

    assert_eq!(values.pop_front(), Some(1));
    values.push(4).unwrap();
    values.push(5).unwrap();
    values.insert(1, 9).unwrap();
    assert_eq!(values.as_slice(), &[2, 9, 3, 4, 5]);

    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2]);
    let mut suffix = StableVec::with_capacity(2).unwrap();
    suffix.extend([3, 4]);
    values.append(&mut suffix);

    assert_eq!(values.as_slice(), &[1, 2, 3, 4]);
    assert!(suffix.is_empty());
}

#[test]
fn stable_vec_remove_preserves_order_and_rejects_out_of_bounds() {
    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2, 3, 4]);

    assert_eq!(values.remove(1), Some(2));
    assert_eq!(values.as_slice(), &[1, 3, 4]);
    assert_eq!(values.remove(3), None);
    assert_eq!(values.as_slice(), &[1, 3, 4]);
}

#[test]
fn stable_vec_remove_drops_only_the_removed_value() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct DropCount(Rc<Cell<usize>>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Rc::new(Cell::new(0));
    let mut values = StableVec::with_capacity(3).unwrap();
    for _ in 0..3 {
        values.push(DropCount(drops.clone())).unwrap();
    }

    drop(values.remove(1));
    assert_eq!(drops.get(), 1);
    drop(values);
    assert_eq!(drops.get(), 3);
}

#[test]
fn stable_vec_supports_zero_sized_and_over_aligned_elements() {
    struct Empty;
    #[repr(align(128))]
    struct Aligned(u8);

    let mut empty = StableVec::with_capacity(3).unwrap();
    empty.extend([Empty, Empty, Empty]);
    assert_eq!(empty.len(), 3);
    assert_eq!(empty.iter().count(), 3);
    assert_eq!(empty.iter().rev().count(), 3);
    assert!(empty.pop().is_some());

    let mut aligned = StableVec::with_capacity(2).unwrap();
    aligned.push(Aligned(42)).unwrap();
    assert_eq!(aligned.as_slice().as_ptr() as usize % 128, 0);
    assert_eq!(aligned[0].0, 42);
}

#[test]
fn stable_vec_drops_moved_elements_once() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct DropCount(Rc<Cell<usize>>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Rc::new(Cell::new(0));
    {
        let mut values = StableVec::with_capacity(4).unwrap();
        values.push(DropCount(drops.clone())).unwrap();
        values.push(DropCount(drops.clone())).unwrap();
        values.push(DropCount(drops.clone())).unwrap();
        drop(values.pop_front());
        values.insert(1, DropCount(drops.clone())).unwrap();

        let mut suffix = StableVec::with_capacity(1).unwrap();
        suffix.push(DropCount(drops.clone())).unwrap();
        values.append(&mut suffix);
    }

    assert_eq!(drops.get(), 5);
}
