use std::io;
use std::iter::FusedIterator;
use std::ops::Range;

pub use soa_derive::Soa;

const DEFAULT_CAPACITY: usize = 1024;

/// Operations generated for a type-of-arrays representation of a row type.
pub trait Soa: Sized {
    type Storage;

    fn create_storage(capacity: usize) -> io::Result<Self::Storage>;
    fn prepare_append(storage: &mut Self::Storage, additional: usize) -> io::Result<()>;
    fn push_to(storage: &mut Self::Storage, value: Self) -> io::Result<()>;
    fn insert(storage: &mut Self::Storage, index: usize, value: Self) -> io::Result<()>;
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

pub struct ECS<T: Soa> {
    storage: T::Storage,
    len: usize,
    capacity: usize,
}

impl<T: Soa> ECS<T> {
    /// Creates storage with the default capacity, panicking if reservation fails.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY).expect("failed to allocate ECS storage")
    }

    /// Creates per-field storage sized for `capacity` values.
    pub fn with_capacity(capacity: usize) -> io::Result<Self> {
        Ok(Self {
            storage: T::create_storage(capacity)?,
            len: 0,
            capacity,
        })
    }

    /// Appends one row to every field column.
    pub fn push(&mut self, value: T) -> io::Result<()> {
        if self.len == self.capacity {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "ECS capacity exceeded",
            ));
        }
        T::push_to(&mut self.storage, value)?;
        self.len += 1;
        Ok(())
    }

    /// Appends rows until the iterator ends or a column runs out of capacity.
    /// Rows appended before an error remain in the ECS.
    pub fn try_extend<I>(&mut self, values: I) -> io::Result<()>
    where
        I: IntoIterator<Item = T>,
    {
        for value in values {
            self.push(value)?;
        }
        Ok(())
    }

    /// Moves all rows from `other` to the end of this ECS.
    /// Reserves destination capacity before moving any rows from `other`.
    pub fn append(&mut self, other: &mut Self) -> io::Result<()> {
        if other.len > self.capacity - self.len {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "ECS capacity exceeded",
            ));
        }
        T::prepare_append(&mut self.storage, other.len)?;

        while let Some(value) = other.pop_front() {
            self.push(value)?;
        }
        Ok(())
    }

    /// Inserts a row at `index`, shifting later rows while preserving their order.
    pub fn insert(&mut self, index: usize, value: T) -> io::Result<()> {
        if index > self.len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ECS insertion index out of bounds",
            ));
        }
        if self.len == self.capacity {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "ECS capacity exceeded",
            ));
        }
        T::insert(&mut self.storage, index, value)?;
        self.len += 1;
        Ok(())
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
    pub fn iter_cloned(&self) -> ECSIter<'_, T>
    where
        T: SoaClone,
    {
        ECSIter {
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
                .expect("ECS columns must have equal lengths");
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
                .expect("ECS columns must have equal lengths");
            if predicate(&row) {
                index += 1;
            } else {
                self.swap_remove(index);
            }
        }
    }

    /// Returns the field columns backing this ECS.
    pub fn storage(&self) -> &T::Storage {
        &self.storage
    }
}

pub struct ECSIter<'a, T: SoaClone> {
    ecs: &'a ECS<T>,
    indices: Range<usize>,
}

impl<T: SoaClone> Iterator for ECSIter<'_, T> {
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

impl<T: SoaClone> DoubleEndedIterator for ECSIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.indices
            .next_back()
            .and_then(|index| T::clone_at(&self.ecs.storage, index))
    }
}

impl<T: SoaClone> ExactSizeIterator for ECSIter<'_, T> {}
impl<T: SoaClone> FusedIterator for ECSIter<'_, T> {}

pub struct ECSIntoIter<T: Soa> {
    ecs: ECS<T>,
}

impl<T: Soa> Iterator for ECSIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.ecs.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.ecs.len, Some(self.ecs.len))
    }
}

impl<T: Soa> DoubleEndedIterator for ECSIntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ecs.pop()
    }
}

impl<T: Soa> ExactSizeIterator for ECSIntoIter<T> {}
impl<T: Soa> FusedIterator for ECSIntoIter<T> {}

impl<T: Soa> IntoIterator for ECS<T> {
    type Item = T;
    type IntoIter = ECSIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        ECSIntoIter { ecs: self }
    }
}

impl<T: Soa> Default for ECS<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Soa> Clone for ECS<T>
where
    T::Storage: Clone,
{
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            len: self.len,
            capacity: self.capacity,
        }
    }
}

impl<T: Soa> PartialEq for ECS<T>
where
    T::Storage: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.capacity == other.capacity && self.storage == other.storage
    }
}

impl<T: Soa> Eq for ECS<T> where T::Storage: Eq {}

impl<T: Soa> std::fmt::Debug for ECS<T>
where
    T::Storage: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ECS")
            .field("storage", &self.storage)
            .field("len", &self.len)
            .field("capacity", &self.capacity)
            .finish()
    }
}

impl<T: Soa> std::iter::FromIterator<T> for ECS<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let rows: Vec<T> = iter.into_iter().collect();
        let mut ecs = Self::with_capacity(rows.len()).expect("failed to allocate ECS storage");
        ecs.try_extend(rows)
            .expect("new ECS capacity should fit collected rows");
        ecs
    }
}

impl<T: Soa> Extend<T> for ECS<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.try_extend(iter).expect("ECS capacity exceeded");
    }
}
