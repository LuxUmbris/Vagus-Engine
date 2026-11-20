//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use vagus::essential::collections::StableVec;

fn collect<T: Copy>(values: &StableVec<T>) -> Vec<T> {
    values.iter().copied().collect()
}

#[test]
fn stable_vec_default_reserves_eight_gibibytes_on_64_bit_virtual_memory_targets() {
    let values = StableVec::<u8>::try_new().expect("reserve default StableVec region");
    let expected_bytes: u64 = if cfg!(all(target_pointer_width = "64", any(unix, windows))) {
        8u64 * 1024 * 1024 * 1024
    } else {
        1024u64 * 1024 * 1024
    };

    assert!(values.is_empty());
    assert_eq!(values.capacity() as u64, expected_bytes);
}

#[test]
fn stable_vec_default_zero_sized_type_uses_unbounded_logical_capacity() {
    let mut values = StableVec::<()>::try_new().expect("create default zero-sized StableVec");

    assert_eq!(values.capacity(), usize::MAX);
    values
        .push(())
        .expect("zero-sized element needs no backing storage");
    assert_eq!(values.len(), 1);
    assert_eq!(values.pop(), Some(()));
}

#[test]
fn stable_vec_try_with_capacity_reports_capacity_overflow() {
    let error = StableVec::<u64>::try_with_capacity(usize::MAX)
        .err()
        .expect("overflowing capacity should be reported");

    assert_eq!(error.kind(), std::io::ErrorKind::Other);
}

#[test]
fn small_stable_vec_accepts_pushes_within_capacity() {
    let values = StableVec::with_capacity(2).expect("reserve two elements");

    values.push(10_u32).unwrap();
    values.push(20_u32).unwrap();

    assert_eq!(collect(&values), [10, 20]);
    assert!(values.push(30).is_err());
    assert!(values.insert(1, 30).is_err());
    assert_eq!(collect(&values), [10, 20]);
}

#[test]
fn stable_vec_push_reuses_removed_slots_without_moving_live_addresses() {
    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2, 3]);
    assert_eq!(values.pop_front(), Some(1));
    let first_address = values.get(0).unwrap() as *const i32;

    values.push(4).unwrap();
    values.push(5).unwrap();
    assert!(values.push(6).is_err());

    assert_eq!(values.get(0).unwrap() as *const i32, first_address);
    assert_eq!(collect(&values), [2, 3, 4, 5]);
}

#[test]
fn stable_vec_reservation_counts_reusable_slots_without_moving_live_elements() {
    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2, 3]);
    assert_eq!(values.pop_front(), Some(1));
    let first_address = values.get(0).unwrap() as *const i32;

    values.prepare_additional(2).unwrap();
    assert_eq!(
        values.prepare_additional(3).unwrap_err().kind(),
        std::io::ErrorKind::OutOfMemory
    );

    assert_eq!(values.get(0).unwrap() as *const i32, first_address);
    assert_eq!(collect(&values), [2, 3]);
}

#[test]
fn stable_vec_handles_front_removal_insertion_and_append_in_order() {
    let mut values = StableVec::with_capacity(5).unwrap();
    values.extend([1, 2, 3]);

    assert_eq!(values.pop_front(), Some(1));
    values.push(4).unwrap();
    values.push(5).unwrap();
    values.insert(1, 9).unwrap();
    assert_eq!(collect(&values), [2, 9, 3, 4, 5]);

    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2]);
    let mut suffix = StableVec::with_capacity(2).unwrap();
    suffix.extend([3, 4]);
    values.append(&mut suffix);

    assert_eq!(collect(&values), [1, 2, 3, 4]);
    assert!(suffix.is_empty());
}

#[test]
fn stable_vec_preserves_live_addresses_across_structural_edits_and_append() {
    let mut values = StableVec::with_capacity(8).unwrap();
    values.extend([10, 20, 30, 40]);
    let thirty_address = values.get(2).unwrap() as *const i32;

    values.insert(0, 5).unwrap();
    assert_eq!(values.get(3).unwrap() as *const i32, thirty_address);

    values.remove(1).unwrap();
    assert_eq!(values.get(2).unwrap() as *const i32, thirty_address);

    let forty_address = values.get(3).unwrap() as *const i32;
    values.swap_remove(0).unwrap();
    assert_eq!(values.get(0).unwrap() as *const i32, forty_address);
    assert_eq!(values.get(2).unwrap() as *const i32, thirty_address);

    let mut suffix = StableVec::with_capacity(2).unwrap();
    suffix.extend([50, 60]);
    let fifty_address = suffix.get(0).unwrap() as *const i32;
    values.append(&mut suffix);
    assert_eq!(values.get(2).unwrap() as *const i32, thirty_address);
    assert_eq!(values.get(3).unwrap() as *const i32, fifty_address);
}

#[test]
fn stable_vec_cross_chunk_insert_and_remove_preserve_survivor_addresses() {
    let mut values = StableVec::with_capacity(2).unwrap();
    values.extend([10, 20]);
    let ten_address = values.get(0).unwrap() as *const i32;
    let twenty_address = values.get(1).unwrap() as *const i32;

    values.grow_to(4).unwrap();
    values.insert(1, 15).unwrap();

    assert_eq!(values.get(0).unwrap() as *const i32, ten_address);
    assert_eq!(values.get(2).unwrap() as *const i32, twenty_address);
    assert_eq!(collect(&values), [10, 15, 20]);

    assert_eq!(values.remove(1), Some(15));
    assert_eq!(values.get(0).unwrap() as *const i32, ten_address);
    assert_eq!(values.get(1).unwrap() as *const i32, twenty_address);
    assert_eq!(collect(&values), [10, 20]);
}

#[test]
fn stable_vec_handle_tracks_live_elements_through_order_changes_and_append() {
    let mut source = StableVec::with_capacity(2).unwrap();
    source.extend([10, 20]);
    source.grow_to(4).unwrap();
    let handle = source.get_handle(1).unwrap();

    source.insert(1, 15).unwrap();
    assert_eq!(source.get_by_handle(&handle), Some(&20));
    *source.get_mut_by_handle(&handle).unwrap() = 21;
    assert_eq!(source.remove(0), Some(10));
    assert_eq!(source.get_by_handle(&handle), Some(&21));

    let destination = StableVec::with_capacity(1).unwrap();
    destination.push(99).unwrap();
    destination.append(&mut source);

    assert!(source.get_by_handle(&handle).is_none());
    assert_eq!(destination.get_by_handle(&handle), Some(&21));
}

#[test]
fn stable_vec_handles_from_later_chunks_survive_order_changes_and_append() {
    let mut source = StableVec::with_capacity(1).unwrap();
    source.push(10).unwrap();
    source.grow_to(4).unwrap();
    source.extend([20, 30, 40]);
    source.grow_to(6).unwrap();
    source.push(50).unwrap();

    let second_chunk_handle = source.get_handle(3).unwrap();
    let third_chunk_handle = source.get_handle(4).unwrap();
    let second_chunk_address = source.get_by_handle(&second_chunk_handle).unwrap() as *const i32;
    let third_chunk_address = source.get_by_handle(&third_chunk_handle).unwrap() as *const i32;
    source.insert(1, 15).unwrap();
    assert_eq!(source.get_by_handle(&second_chunk_handle), Some(&40));
    assert_eq!(source.get_by_handle(&third_chunk_handle), Some(&50));
    assert_eq!(
        source.get_by_handle(&second_chunk_handle).unwrap() as *const i32,
        second_chunk_address
    );
    assert_eq!(
        source.get_by_handle(&third_chunk_handle).unwrap() as *const i32,
        third_chunk_address
    );

    assert_eq!(source.swap_remove(0), Some(10));
    assert_eq!(source.get_by_handle(&second_chunk_handle), Some(&40));
    assert_eq!(source.get_by_handle(&third_chunk_handle), Some(&50));

    let destination = StableVec::with_capacity(2).unwrap();
    destination.push(99).unwrap();
    destination.append(&mut source);

    assert!(source.get_by_handle(&second_chunk_handle).is_none());
    assert!(source.get_by_handle(&third_chunk_handle).is_none());
    assert_eq!(destination.get_by_handle(&second_chunk_handle), Some(&40));
    assert_eq!(destination.get_by_handle(&third_chunk_handle), Some(&50));
    assert_eq!(
        destination.get_by_handle(&second_chunk_handle).unwrap() as *const i32,
        second_chunk_address
    );
    assert_eq!(
        destination.get_by_handle(&third_chunk_handle).unwrap() as *const i32,
        third_chunk_address
    );
}

#[test]
fn stable_vec_handle_rejects_foreign_vectors_and_reused_slots() {
    let mut values = StableVec::with_capacity(2).unwrap();
    values.extend([10, 20]);
    let handle = values.get_handle(1).unwrap();

    let foreign = StableVec::from([10, 20]);
    let cloned = values.clone();
    assert!(foreign.get_by_handle(&handle).is_none());
    assert!(cloned.get_by_handle(&handle).is_none());

    assert_eq!(values.remove(1), Some(20));
    values.push(30).unwrap();
    assert_eq!(values.get_by_handle(&handle), None);
    assert_eq!(collect(&values), [10, 30]);
}

#[test]
fn stable_vec_growth_adds_storage_without_moving_live_elements() {
    let mut values = StableVec::with_capacity(1).unwrap();
    values.push(7_i32).unwrap();
    let address = values.get(0).unwrap() as *const i32;

    values.grow_to(4).unwrap();
    values.extend([8, 9, 10]);

    assert_eq!(values.get(0).unwrap() as *const i32, address);
    assert_eq!(collect(&values), [7, 8, 9, 10]);
}

#[test]
fn stable_vec_truncate_preserves_addresses_of_retained_elements() {
    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([10, 20, 30, 40]);
    let first_address = values.get(0).unwrap() as *const i32;
    let second_address = values.get(1).unwrap() as *const i32;

    assert!(values.truncate(2));

    assert_eq!(values.get(0).unwrap() as *const i32, first_address);
    assert_eq!(values.get(1).unwrap() as *const i32, second_address);
    assert_eq!(collect(&values), [10, 20]);
    assert!(!values.truncate(3));
    assert_eq!(collect(&values), [10, 20]);
}

#[test]
fn stable_vec_truncate_keeps_prefix_valid_when_a_destructor_panics() {
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    struct DropProbe {
        id: usize,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
            assert_ne!(self.id, 3, "intentional destructor panic");
        }
    }

    let drops = Rc::new(Cell::new(0));
    let mut values = StableVec::with_capacity(4).unwrap();
    for id in 0..4 {
        values
            .push(DropProbe {
                id,
                drops: Rc::clone(&drops),
            })
            .unwrap();
    }

    let result = catch_unwind(AssertUnwindSafe(|| values.truncate(2)));

    assert!(result.is_err());
    assert_eq!(
        values.iter().map(|value| value.id).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(drops.get(), 1);

    assert!(values.truncate(1));
    assert_eq!(values.iter().map(|value| value.id).collect::<Vec<_>>(), [0]);
    assert_eq!(drops.get(), 3);

    drop(values);
    assert_eq!(drops.get(), 4);
}

#[test]
fn stable_vec_mut_iter_can_mix_front_and_back_without_aliasing_slots() {
    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2, 3, 4]);

    let mut iter = values.iter_mut();
    *iter.next().unwrap() = 10;
    *iter.next_back().unwrap() = 40;
    *iter.next_back().unwrap() = 30;
    *iter.next().unwrap() = 20;

    assert_eq!(collect(&values), [10, 20, 30, 40]);
}

#[test]
fn stable_vec_remove_preserves_order_and_rejects_out_of_bounds() {
    let mut values = StableVec::with_capacity(4).unwrap();
    values.extend([1, 2, 3, 4]);

    assert_eq!(values.remove(1), Some(2));
    assert_eq!(collect(&values), [1, 3, 4]);
    assert_eq!(values.remove(3), None);
    assert_eq!(collect(&values), [1, 3, 4]);
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

    let aligned = StableVec::with_capacity(2).unwrap();
    aligned.push(Aligned(42)).unwrap();
    assert_eq!(&aligned[0] as *const Aligned as usize % 128, 0);
    assert_eq!(aligned[0].0, 42);
}

#[test]
fn stable_vec_reuses_removed_zero_sized_slot_at_max_capacity() {
    let mut values = StableVec::<()>::with_capacity(usize::MAX)
        .expect("zero-sized elements need no backing reservation");

    values.push(()).unwrap();
    let _ = values.pop().unwrap();
    values
        .push(())
        .expect("a removed slot remains available at maximal capacity");
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

#[test]
fn stable_vec_drop_continues_after_an_element_destructor_panics() {
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    struct DropProbe {
        drops: Rc<Cell<usize>>,
        panic_on_drop: bool,
    }
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
            assert!(!self.panic_on_drop, "intentional destructor panic");
        }
    }

    let drops = Rc::new(Cell::new(0));
    let values = StableVec::with_capacity(2).unwrap();
    values
        .push(DropProbe {
            drops: drops.clone(),
            panic_on_drop: true,
        })
        .unwrap();
    values
        .push(DropProbe {
            drops: drops.clone(),
            panic_on_drop: false,
        })
        .unwrap();

    let result = catch_unwind(AssertUnwindSafe(|| drop(values)));

    assert!(result.is_err());
    assert_eq!(drops.get(), 2);
}

#[test]
fn stable_vec_references_survive_non_destructive_structural_edits() {
    let mut values = StableVec::with_capacity(8).unwrap();
    values.extend([10, 20, 30]);
    let stable_reference = values.get(1).unwrap();

    values.grow_to(12).unwrap();
    values.insert(0, 5).unwrap();
    values.push(40).unwrap();
    assert_eq!(*stable_reference, 20);

    let mut suffix = StableVec::with_capacity(1).unwrap();
    suffix.push(50).unwrap();
    values.append(&mut suffix);

    assert_eq!(*stable_reference, 20);
    assert_eq!(collect(&values), [5, 10, 20, 30, 40, 50]);
}
