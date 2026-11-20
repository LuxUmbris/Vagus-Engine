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

use vagus::ecs::{ECS, Soa, SoaClone};

#[derive(Soa, PartialEq, Eq)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Soa)]
struct Pair(i32, u8);

#[derive(Soa)]
struct Empty;

#[repr(align(4096))]
#[derive(Soa)]
struct PaddedValue {
    value: u8,
}

#[derive(Soa)]
struct ZeroSizedColumn {
    marker: std::marker::PhantomData<()>,
}

#[derive(Soa)]
struct Generic<T> {
    value: T,
}

#[test]
fn ecs_pushes_struct_fields_into_columns() {
    let mut entities = ECS::<Position>::with_capacity(2).unwrap();
    entities.push(Position { x: 3, y: 8 }).unwrap();
    entities.push(Position { x: -4, y: 11 }).unwrap();

    assert_eq!(entities.len(), 2);
    assert_eq!(entities.capacity(), 2);
    let columns = entities.storage();
    assert_eq!(columns.x.get(0), Some(&3));
    assert_eq!(columns.y.get(1), Some(&11));
}

#[test]
fn ecs_try_replace_updates_all_columns_and_returns_rejected_input() {
    let mut entities = ECS::<Position>::with_capacity(2).unwrap();
    entities.push(Position { x: 3, y: 8 }).unwrap();
    entities.push(Position { x: -4, y: 11 }).unwrap();

    let replaced = match entities.try_replace(1, Position { x: 7, y: -9 }) {
        Ok(row) => row,
        Err(_) => panic!("valid row replacement was rejected"),
    };
    assert_eq!((replaced.x, replaced.y), (-4, 11));
    assert_eq!(
        entities.get_cloned(1).map(|row| (row.x, row.y)),
        Some((7, -9))
    );

    let rejected = match entities.try_replace(2, Position { x: 12, y: 13 }) {
        Err(row) => row,
        Ok(_) => panic!("out-of-bounds row replacement succeeded"),
    };
    assert_eq!((rejected.x, rejected.y), (12, 13));
    assert_eq!(entities.len(), 2);
    assert_eq!(
        entities.get_cloned(1).map(|row| (row.x, row.y)),
        Some((7, -9))
    );
    assert_eq!(entities.storage().x.len(), 2);
    assert_eq!(entities.storage().y.len(), 2);

    let mut empty = ECS::<Empty>::with_capacity(1).unwrap();
    empty.push(Empty).unwrap();
    assert!(empty.try_replace(0, Empty).is_ok());
    assert!(empty.try_replace(1, Empty).is_err());
    assert_eq!(empty.len(), 1);
}

#[test]
fn default_ecs_reserves_approximately_32_gib_for_rows() {
    let mut entities = ECS::<Position>::new();
    let target_bytes = if cfg!(all(target_pointer_width = "64", any(unix, windows))) {
        32_u64 * 1024 * 1024 * 1024
    } else {
        1024_u64 * 1024 * 1024
    };
    let reserved_row_bytes = (entities.capacity() as u64)
        .checked_mul(std::mem::size_of::<Position>() as u64)
        .unwrap();

    assert!(reserved_row_bytes > 0);
    assert!(reserved_row_bytes <= target_bytes);

    entities.push(Position { x: 7, y: -7 }).unwrap();
    assert_eq!(
        entities.get_cloned(0).map(|row| (row.x, row.y)),
        Some((7, -7))
    );
}

#[test]
fn default_ecs_capacity_accounts_for_soa_column_bytes_not_row_padding() {
    let mut entities = ECS::<PaddedValue>::new();
    let target_bytes = if cfg!(all(target_pointer_width = "64", any(unix, windows))) {
        32_u64 * 1024 * 1024 * 1024
    } else {
        1024_u64 * 1024 * 1024
    };

    let reserved_column_bytes = (entities.capacity() as u64)
        .checked_mul(std::mem::size_of::<u8>() as u64)
        .unwrap();
    assert_eq!(
        PaddedValue::STORAGE_ROW_BYTES,
        Some(std::mem::size_of::<u8>())
    );
    assert!(reserved_column_bytes > 0);
    assert!(reserved_column_bytes <= target_bytes);
    entities.push(PaddedValue { value: 7 }).unwrap();
    assert_eq!(entities.storage().value.get(0), Some(&7));
}

#[test]
fn growable_ecs_accepts_a_caller_selected_initial_capacity() {
    let mut entities = ECS::<Position>::with_growable_capacity(4).unwrap();
    assert_eq!(entities.capacity(), 4);

    entities
        .try_extend((0..4).map(|x| Position { x, y: -x }))
        .unwrap();
    entities.push(Position { x: 4, y: -4 }).unwrap();

    assert_eq!(entities.len(), 5);
    assert!(entities.capacity() > 4);
    assert_eq!(entities.get_cloned(4).map(|row| row.x), Some(4));
}

#[test]
fn default_ecs_growth_preserves_insert_and_append_rows() {
    let mut inserted = ECS::<Position>::with_growable_capacity(2).unwrap();
    let initial_capacity = inserted.capacity();
    inserted
        .try_extend([Position { x: 0, y: 0 }, Position { x: 1, y: -1 }])
        .unwrap();
    inserted.insert(0, Position { x: -1, y: 1 }).unwrap();
    assert_eq!(inserted.get_cloned(0).map(|row| row.x), Some(-1));
    assert_eq!(inserted.get_cloned(1).map(|row| row.x), Some(0));
    assert!(inserted.capacity() > initial_capacity);

    let mut appended = ECS::<Position>::with_growable_capacity(2).unwrap();
    let initial_capacity = appended.capacity();
    appended
        .try_extend([Position { x: 0, y: 0 }, Position { x: 1, y: -1 }])
        .unwrap();
    let mut tail = ECS::<Position>::with_capacity(2).unwrap();
    tail.extend([Position { x: -2, y: 2 }, Position { x: -3, y: 3 }]);
    let tail_address = tail.storage().x.get(0).unwrap() as *const i32;
    appended.append(&mut tail).unwrap();
    assert_eq!(
        appended.get_cloned(initial_capacity).map(|row| row.x),
        Some(-2)
    );
    assert_eq!(
        appended.get_cloned(initial_capacity + 1).map(|row| row.x),
        Some(-3)
    );
    assert_eq!(
        appended.storage().x.get(initial_capacity).unwrap() as *const i32,
        tail_address
    );
    assert!(tail.is_empty());
    assert_eq!(tail.capacity(), 2);
    tail.push(Position { x: -4, y: 4 }).unwrap();
    assert_eq!(tail.get_cloned(0).map(|row| row.x), Some(-4));
    assert!(appended.capacity() > initial_capacity);
}

#[test]
fn generated_soa_growth_preserves_live_column_addresses_after_front_removal() {
    let mut entities = ECS::<Position>::with_growable_capacity(2).unwrap();
    entities.extend([Position { x: 1, y: 10 }, Position { x: 2, y: 20 }]);
    assert_eq!(
        entities.pop_front().map(|row| (row.x, row.y)),
        Some((1, 10))
    );

    let addresses = (
        entities.storage().x.get(0).unwrap() as *const i32,
        entities.storage().y.get(0).unwrap() as *const i32,
    );
    let initial_capacity = entities.capacity();

    entities.push(Position { x: 3, y: 30 }).unwrap();
    entities.push(Position { x: 4, y: 40 }).unwrap();

    assert!(entities.capacity() > initial_capacity);
    assert_eq!(
        (
            entities.storage().x.get(0).unwrap() as *const i32,
            entities.storage().y.get(0).unwrap() as *const i32,
        ),
        addresses
    );
    assert_eq!(
        entities.get_cloned(0).map(|row| (row.x, row.y)),
        Some((2, 20))
    );
}

#[test]
fn default_ecs_fieldless_rows_have_no_arbitrary_capacity_limit() {
    let mut entities = ECS::<Empty>::new();

    assert_eq!(entities.capacity(), usize::MAX);
    entities.try_extend((0..2048).map(|_| Empty)).unwrap();

    assert_eq!(entities.len(), 2048);
    assert_eq!(entities.capacity(), usize::MAX);
    assert!(entities.get_cloned(2047).is_some());
}

#[test]
fn default_ecs_zero_sized_columns_have_no_arbitrary_capacity_limit() {
    let mut entities = ECS::<ZeroSizedColumn>::new();

    assert_eq!(entities.capacity(), usize::MAX);
    entities
        .try_extend((0..2048).map(|_| ZeroSizedColumn {
            marker: std::marker::PhantomData,
        }))
        .unwrap();
    assert_eq!(entities.len(), 2048);
    assert!(entities.get_cloned(2047).is_some());
}

#[test]
fn ecs_capacity_error_does_not_partially_append_columns() {
    let mut entities = ECS::<Position>::with_capacity(1).unwrap();
    entities.push(Position { x: 3, y: 8 }).unwrap();
    assert!(entities.push(Position { x: 5, y: 13 }).is_err());

    assert_eq!(entities.len(), 1);
    assert_eq!(entities.storage().x.len(), 1);
    assert_eq!(entities.storage().y.len(), 1);
}

#[test]
fn derive_supports_tuple_unit_and_generic_structs() {
    let mut pair = ECS::<Pair>::with_capacity(1).unwrap();
    pair.push(Pair(7, 2)).unwrap();
    assert_eq!(pair.storage()._0.get(0), Some(&7));
    assert_eq!(pair.storage()._1.get(0), Some(&2));

    let mut empty = ECS::<Empty>::with_capacity(1).unwrap();
    empty.push(Empty).unwrap();
    assert_eq!(empty.len(), 1);
    assert!(empty.push(Empty).is_err());
    assert!(empty.pop().is_some());
    assert!(empty.pop().is_none());

    let mut generic = ECS::<Generic<&str>>::with_capacity(1).unwrap();
    generic.push(Generic { value: "value" }).unwrap();
    assert_eq!(generic.storage().value.get(0), Some(&"value"));
}

#[test]
fn unit_soa_generated_operations_track_rows_and_capacity() {
    let mut storage = Empty::create_storage(2).unwrap();
    assert!(Empty::clone_at(&storage, 0).is_none());
    Empty::prepare_append(&mut storage, 2).unwrap();
    assert!(Empty::prepare_append(&mut storage, 3).is_err());

    Empty::push_to(&mut storage, Empty).unwrap();
    Empty::insert(&mut storage, 0, Empty).unwrap();
    assert!(Empty::push_to(&mut storage, Empty).is_err());
    assert!(Empty::clone_at(&storage, 1).is_some());
    assert!(Empty::clone_at(&storage, 2).is_none());

    assert!(Empty::swap_remove(&mut storage, 0).is_some());
    assert!(Empty::remove(&mut storage, 0).is_some());
    assert!(Empty::push_to(&mut storage, Empty).is_ok());
    assert!(Empty::push_to(&mut storage, Empty).is_ok());
    assert!(Empty::pop_front(&mut storage).is_some());
    assert!(Empty::pop(&mut storage).is_some());
    assert!(Empty::pop(&mut storage).is_none());
    assert!(Empty::truncate(&mut storage, 0));
    assert!(!Empty::truncate(&mut storage, 1));
    Empty::clear(&mut storage);
    assert!(Empty::push_to(&mut storage, Empty).is_ok());
}

#[test]
fn unit_soa_rejects_growth_below_live_row_count_without_mutation() {
    let mut storage = Empty::create_storage(2).unwrap();
    Empty::push_to(&mut storage, Empty).unwrap();
    Empty::push_to(&mut storage, Empty).unwrap();

    assert!(Empty::grow_storage(&mut storage, 2, 1).is_err());
    assert!(Empty::clone_at(&storage, 1).is_some());
    assert!(Empty::push_to(&mut storage, Empty).is_err());
}

#[test]
fn derived_soa_rejects_growth_below_live_rows_without_mutation() {
    let mut storage = Position::create_storage(2).unwrap();
    Position::push_to(&mut storage, Position { x: 3, y: 8 }).unwrap();
    Position::push_to(&mut storage, Position { x: -4, y: 11 }).unwrap();
    let initial_capacities = (storage.x.capacity(), storage.y.capacity());

    assert!(Position::grow_storage(&mut storage, 2, 1).is_err());
    assert_eq!(
        (storage.x.capacity(), storage.y.capacity()),
        initial_capacities
    );
    assert_eq!(storage.x.len(), 2);
    assert_eq!(storage.y.len(), 2);
    assert_eq!(
        Position::clone_at(&storage, 0).map(|row| (row.x, row.y)),
        Some((3, 8))
    );
    assert_eq!(
        Position::clone_at(&storage, 1).map(|row| (row.x, row.y)),
        Some((-4, 11))
    );
}

#[test]
fn ecs_row_operations_keep_soa_columns_aligned() {
    let mut entities = ECS::<Position>::with_capacity(5).unwrap();
    entities
        .try_extend([
            Position { x: 1, y: 10 },
            Position { x: 2, y: 20 },
            Position { x: 3, y: 30 },
        ])
        .unwrap();

    entities.insert(1, Position { x: 9, y: 90 }).unwrap();
    let removed = entities.remove(2).unwrap();
    assert_eq!((removed.x, removed.y), (2, 20));
    let swapped = entities.swap_remove(0).unwrap();
    assert_eq!((swapped.x, swapped.y), (1, 10));
    let front = entities.pop_front().unwrap();
    assert_eq!((front.x, front.y), (3, 30));
    let back = entities.pop().unwrap();
    assert_eq!((back.x, back.y), (9, 90));
    assert!(entities.is_empty());
}

#[test]
fn ecs_cloned_rows_iterators_and_collection_traits_work() {
    let mut entities = ECS::<Position>::with_capacity(3).unwrap();
    entities.extend([Position { x: 4, y: 40 }, Position { x: 5, y: 50 }]);
    assert_eq!(entities.get_cloned(1).map(|p| (p.x, p.y)), Some((5, 50)));
    let mut cloned_rows = entities.iter_cloned();
    assert_eq!(cloned_rows.len(), 2);
    assert_eq!(cloned_rows.next_back().map(|p| (p.x, p.y)), Some((5, 50)));
    assert_eq!(cloned_rows.next().map(|p| (p.x, p.y)), Some((4, 40)));
    assert!(cloned_rows.next().is_none());

    let cloned = entities.clone();
    assert_eq!(entities, cloned);
    assert!(format!("{entities:?}").contains("PositionStorage"));
    assert_eq!(
        entities
            .into_iter()
            .rev()
            .map(|p| (p.x, p.y))
            .collect::<Vec<_>>(),
        [(5, 50), (4, 40)]
    );
}

#[test]
fn ecs_try_extend_reports_capacity_and_collection_builds_exact_capacity() {
    let mut bounded = ECS::<Position>::with_capacity(1).unwrap();
    assert!(
        bounded
            .try_extend([Position { x: 1, y: 10 }, Position { x: 2, y: 20 }])
            .is_err()
    );
    assert_eq!(bounded.len(), 1);
    assert_eq!(bounded.get_cloned(0).map(|p| (p.x, p.y)), Some((1, 10)));

    let collected: ECS<Position> = [Position { x: 7, y: 70 }, Position { x: 8, y: 80 }]
        .into_iter()
        .collect();
    assert_eq!(collected.capacity(), 2);
    assert_eq!(collected.len(), 2);
}

#[test]
fn truncate_and_clear_drop_rows_and_preserve_capacity() {
    let mut entities = ECS::<Position>::with_capacity(4).unwrap();
    entities.extend((0..3).map(|x| Position { x, y: x * 10 }));
    assert!(!entities.truncate(4));
    assert!(entities.truncate(1));
    assert_eq!(entities.get_cloned(0).map(|p| (p.x, p.y)), Some((0, 0)));
    entities.clear();
    assert_eq!(entities.len(), 0);
    assert_eq!(entities.capacity(), 4);
}

#[test]
fn append_queries_and_retain_preserve_rows_and_capacity_rules() {
    let mut left = ECS::<Position>::with_capacity(4).unwrap();
    left.extend([
        Position { x: 0, y: 0 },
        Position { x: 1, y: 10 },
        Position { x: 2, y: 20 },
    ]);
    left.pop_front();
    left.pop_front();
    let mut right = ECS::<Position>::with_capacity(2).unwrap();
    right.extend([Position { x: 3, y: 30 }, Position { x: 4, y: 40 }]);

    left.append(&mut right).unwrap();
    assert!(right.is_empty());
    assert!(left.contains(&Position { x: 3, y: 30 }));
    assert_eq!(left.position(|row| row.x == 4), Some(2));
    left.retain(|row| row.x % 2 == 0);
    assert_eq!(
        left.iter_cloned().map(|row| row.x).collect::<Vec<_>>(),
        [2, 4]
    );
    left.retain_unordered(|row| row.x != 2);
    assert_eq!(left.iter_cloned().map(|row| row.x).collect::<Vec<_>>(), [4]);

    let mut full = ECS::<Position>::with_capacity(1).unwrap();
    full.push(Position { x: 9, y: 90 }).unwrap();
    let mut tail = ECS::<Position>::with_capacity(1).unwrap();
    tail.push(Position { x: 10, y: 100 }).unwrap();
    assert!(full.append(&mut tail).is_err());
    assert_eq!(full.len(), 1);
    assert_eq!(tail.len(), 1);
}
