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

use std::io;
use std::iter::FusedIterator;
use std::ops::Range;

pub use soa_derive::Soa;

const DEFAULT_CAPACITY: usize = 1024;
#[cfg(all(target_pointer_width = "64", any(unix, windows)))]
const DEFAULT_RESERVATION_BYTES: usize = 32 * 1024 * 1024 * 1024;
#[cfg(not(all(target_pointer_width = "64", any(unix, windows))))]
const DEFAULT_RESERVATION_BYTES: usize = 1024 * 1024 * 1024;

fn capacity_for_storage_row_bytes(row_bytes: usize) -> usize {
    (DEFAULT_RESERVATION_BYTES / row_bytes).max(1)
}

fn default_capacity<T: Soa>() -> usize {
    if !T::HAS_COLUMNS {
        usize::MAX
    } else {
        let row_bytes = match T::STORAGE_ROW_BYTES {
            Some(0) => return usize::MAX,
            Some(bytes) => bytes,
            None => {
                let row_size = std::mem::size_of::<T>();
                if row_size == 0 {
                    return DEFAULT_CAPACITY;
                }
                row_size
            }
        };
        capacity_for_storage_row_bytes(row_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_RESERVATION_BYTES, capacity_for_storage_row_bytes};

    #[test]
    fn default_row_capacity_targets_the_configured_reservation() {
        assert_eq!(
            capacity_for_storage_row_bytes(8) * 8,
            DEFAULT_RESERVATION_BYTES
        );
        assert_eq!(capacity_for_storage_row_bytes(1), DEFAULT_RESERVATION_BYTES);
        assert_eq!(
            capacity_for_storage_row_bytes(DEFAULT_RESERVATION_BYTES + 1),
            1
        );
    }
}

/// Operations generated for a ToA representation of a row type.
pub trait Soa: Sized {
    type Storage;
    const HAS_COLUMNS: bool = true;
    const STORAGE_ROW_BYTES: Option<usize> = None;

    fn create_storage(capacity: usize) -> io::Result<Self::Storage>;
    /// Builds larger storage while preserving its rows, or reports unsupported growth.
    fn grow_storage(
        storage: &mut Self::Storage,
        len: usize,
        capacity: usize,
    ) -> io::Result<Self::Storage> {
        let _ = (storage, len, capacity);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SoA storage does not support growth",
        ))
    }
    fn prepare_append(storage: &mut Self::Storage, additional: usize) -> io::Result<()>;
    fn append_storage(
        storage: &mut Self::Storage,
        other: &mut Self::Storage,
        additional: usize,
        other_capacity: usize,
    ) -> io::Result<()> {
        let _ = other_capacity;
        Self::prepare_append(storage, additional)?;
        for _ in 0..additional {
            let value = Self::pop_front(other).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SoA source length is inconsistent",
                )
            })?;
            Self::push_to(storage, value)?;
        }
        Ok(())
    }
    fn push_to(storage: &mut Self::Storage, value: Self) -> io::Result<()>;
    fn insert(storage: &mut Self::Storage, index: usize, value: Self) -> io::Result<()>;
    fn replace_at(storage: &mut Self::Storage, index: usize, value: Self) -> Result<Self, Self> {
        let _ = (storage, index);
        Err(value)
    }
    fn remove(storage: &mut Self::Storage, index: usize) -> Option<Self>;
    fn swap_remove(storage: &mut Self::Storage, index: usize) -> Option<Self>;
    fn pop(storage: &mut Self::Storage) -> Option<Self>;
    fn pop_front(storage: &mut Self::Storage) -> Option<Self>;
    fn truncate(storage: &mut Self::Storage, len: usize) -> bool;
    fn clear(storage: &mut Self::Storage);
}

/// Adds row cloning for structs whose individual fields implement `Clone`.
pub trait SoaClone: Soa {
    fn clone_at(storage: &Self::Storage, index: usize) -> Option<Self>;
}

pub struct Ecs<T: Soa> {
    storage: T::Storage,
    len: usize,
    capacity: usize,
    growable: bool,
}

impl<T: Soa> Ecs<T> {
    /// Creates growable storage with a virtual reservation for about 32 GiB of row data on
    /// 64-bit VM targets. Pages are committed on demand; smaller capacities are tried if needed.
    /// Derived fieldless rows have `usize::MAX` logical capacity because they store no columns.
    pub fn new() -> Self {
        let mut capacity = default_capacity::<T>();
        loop {
            match Self::with_capacity_and_growth(capacity, true) {
                Ok(entities) => return entities,
                Err(_error) if capacity > 1 => capacity = (capacity / 2).max(1),
                Err(error) => panic!("failed to allocate Ecs storage: {error}"),
            }
        }
    }

    /// Creates fixed-capacity per-field storage sized for `capacity` values.
    pub fn with_capacity(capacity: usize) -> io::Result<Self> {
        Self::with_capacity_and_growth(capacity, false)
    }

    /// Creates growable per-field storage initially sized for `capacity` rows.
    pub fn with_growable_capacity(capacity: usize) -> io::Result<Self> {
        Self::with_capacity_and_growth(capacity, true)
    }

    fn with_capacity_and_growth(capacity: usize, growable: bool) -> io::Result<Self> {
        Ok(Self {
            storage: T::create_storage(capacity)?,
            len: 0,
            capacity,
            growable,
        })
    }

    fn ensure_capacity(&mut self, required: usize) -> io::Result<()> {
        if required <= self.capacity {
            return Ok(());
        }
        if !self.growable {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "Ecs capacity exceeded",
            ));
        }

        let doubled = self.capacity.checked_mul(2).unwrap_or(usize::MAX);
        let capacity = required.max(doubled).max(1);
        let storage = T::grow_storage(&mut self.storage, self.len, capacity)?;
        self.storage = storage;
        self.capacity = capacity;
        Ok(())
    }

    /// Appends one row to every field column.
    pub fn push(&mut self, value: T) -> io::Result<()> {
        let required = self
            .len
            .checked_add(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::OutOfMemory, "Ecs length overflow"))?;
        self.ensure_capacity(required)?;
        T::push_to(&mut self.storage, value)?;
        self.len += 1;
        Ok(())
    }

    /// Appends rows until the iterator ends or storage allocation fails.
    /// Rows appended before an error remain in the Ecs.
    pub fn try_extend<I>(&mut self, values: I) -> io::Result<()>
    where
        I: IntoIterator<Item = T>,
    {
        for value in values {
            self.push(value)?;
        }
        Ok(())
    }

    /// Moves all rows from `other` to the end of this Ecs.
    /// Reserves destination capacity before moving any rows from `other`.
    pub fn append(&mut self, other: &mut Self) -> io::Result<()> {
        let required = self
            .len
            .checked_add(other.len)
            .ok_or_else(|| io::Error::new(io::ErrorKind::OutOfMemory, "Ecs length overflow"))?;
        self.ensure_capacity(required)?;
        T::append_storage(
            &mut self.storage,
            &mut other.storage,
            other.len,
            other.capacity,
        )?;
        self.len = required;
        other.len = 0;
        Ok(())
    }

    /// Inserts a row at `index`, shifting later rows while preserving their order.
    pub fn insert(&mut self, index: usize, value: T) -> io::Result<()> {
        if index > self.len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Ecs insertion index out of bounds",
            ));
        }
        let required = self
            .len
            .checked_add(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::OutOfMemory, "Ecs length overflow"))?;
        self.ensure_capacity(required)?;
        T::insert(&mut self.storage, index, value)?;
        self.len += 1;
        Ok(())
    }

    /// Replaces row `index`, returning the old row or the input if replacement fails.
    pub fn try_replace(&mut self, index: usize, value: T) -> Result<T, T> {
        if index >= self.len {
            return Err(value);
        }
        T::replace_at(&mut self.storage, index, value)
    }

    /// Removes a row and preserves the order of the remaining rows.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let removed = T::remove(&mut self.storage, index)?;
        self.len -= 1;
        Some(removed)
    }

    /// Removes a row by replacing it with the last row; order is not preserved.
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let removed = T::swap_remove(&mut self.storage, index)?;
        self.len -= 1;
        Some(removed)
    }

    /// Removes and returns the last row.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let value = T::pop(&mut self.storage)?;
        self.len -= 1;
        Some(value)
    }

    /// Removes and returns the first row without shifting the remaining rows.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let value = T::pop_front(&mut self.storage)?;
        self.len -= 1;
        Some(value)
    }

    /// Truncates to `len`; returns false when `len` exceeds the current length.
    pub fn truncate(&mut self, len: usize) -> bool {
        if len > self.len || !T::truncate(&mut self.storage, len) {
            return false;
        }
        self.len = len;
        true
    }

    /// Drops all rows while retaining the reserved column capacity.
    pub fn clear(&mut self) {
        T::clear(&mut self.storage);
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clones a row from each column when all its fields implement `Clone`.
    pub fn get_cloned(&self, index: usize) -> Option<T>
    where
        T: SoaClone,
    {
        if index >= self.len {
            return None;
        }
        T::clone_at(&self.storage, index)
    }

    /// Iterates over cloned rows in insertion order.
    pub fn iter_cloned(&self) -> EcsIter<'_, T>
    where
        T: SoaClone,
    {
        EcsIter {
            ecs: self,
            indices: 0..self.len,
        }
    }

    /// Returns whether any row compares equal to `value`.
    pub fn contains(&self, value: &T) -> bool
    where
        T: SoaClone + PartialEq,
    {
        self.iter_cloned().any(|row| &row == value)
    }

    /// Returns the index of the first cloned row matching `predicate`.
    pub fn position<F>(&self, predicate: F) -> Option<usize>
    where
        T: SoaClone,
        F: FnMut(T) -> bool,
    {
        self.iter_cloned().position(predicate)
    }

    /// Retains matching rows in order. The predicate receives a cloned row.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        T: SoaClone,
        F: FnMut(&T) -> bool,
    {
        let mut index = 0;
        while index < self.len {
            let row = self
                .get_cloned(index)
                .expect("Ecs columns must have equal lengths");
            if predicate(&row) {
                index += 1;
            } else {
                self.remove(index);
            }
        }
    }

    /// Retains matching rows without preserving their order.
    pub fn retain_unordered<F>(&mut self, mut predicate: F)
    where
        T: SoaClone,
        F: FnMut(&T) -> bool,
    {
        let mut index = 0;
        while index < self.len {
            let row = self
                .get_cloned(index)
                .expect("Ecs columns must have equal lengths");
            if predicate(&row) {
                index += 1;
            } else {
                self.swap_remove(index);
            }
        }
    }

    /// Returns the field columns backing this Ecs.
    pub fn storage(&self) -> &T::Storage {
        &self.storage
    }

    pub(crate) fn storage_mut(&mut self) -> &mut T::Storage {
        &mut self.storage
    }
}

pub struct EcsIter<'a, T: SoaClone> {
    ecs: &'a Ecs<T>,
    indices: Range<usize>,
}

impl<T: SoaClone> Iterator for EcsIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.indices
            .next()
            .and_then(|index| T::clone_at(&self.ecs.storage, index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl<T: SoaClone> DoubleEndedIterator for EcsIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.indices
            .next_back()
            .and_then(|index| T::clone_at(&self.ecs.storage, index))
    }
}

impl<T: SoaClone> ExactSizeIterator for EcsIter<'_, T> {}
impl<T: SoaClone> FusedIterator for EcsIter<'_, T> {}

pub struct EcsIntoIter<T: Soa> {
    ecs: Ecs<T>,
}

impl<T: Soa> Iterator for EcsIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.ecs.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.ecs.len, Some(self.ecs.len))
    }
}

impl<T: Soa> DoubleEndedIterator for EcsIntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ecs.pop()
    }
}

impl<T: Soa> ExactSizeIterator for EcsIntoIter<T> {}
impl<T: Soa> FusedIterator for EcsIntoIter<T> {}

impl<T: Soa> IntoIterator for Ecs<T> {
    type Item = T;
    type IntoIter = EcsIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        EcsIntoIter { ecs: self }
    }
}

impl<T: Soa> Default for Ecs<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Soa> Clone for Ecs<T>
where
    T::Storage: Clone,
{
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            len: self.len,
            capacity: self.capacity,
            growable: self.growable,
        }
    }
}

impl<T: Soa> PartialEq for Ecs<T>
where
    T::Storage: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && self.capacity == other.capacity
            && self.growable == other.growable
            && self.storage == other.storage
    }
}

impl<T: Soa> Eq for Ecs<T> where T::Storage: Eq {}

impl<T: Soa> std::fmt::Debug for Ecs<T>
where
    T::Storage: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ecs")
            .field("storage", &self.storage)
            .field("len", &self.len)
            .field("capacity", &self.capacity)
            .finish()
    }
}

impl<T: Soa> std::iter::FromIterator<T> for Ecs<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let rows: Vec<T> = iter.into_iter().collect();
        let mut ecs = Self::with_capacity(rows.len()).expect("failed to allocate Ecs storage");
        ecs.try_extend(rows)
            .expect("new Ecs capacity should fit collected rows");
        ecs
    }
}

impl<T: Soa> Extend<T> for Ecs<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.try_extend(iter).expect("Ecs capacity exceeded");
    }
}
