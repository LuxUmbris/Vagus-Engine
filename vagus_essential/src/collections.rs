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

use super::allocator::Region;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::{Add, Index, IndexMut};
use std::ptr::NonNull;
use std::sync::{Arc, Weak};

// Unix and Windows regions reserve address space without eagerly committing it.
#[cfg(all(target_pointer_width = "64", any(unix, windows)))]
const DEFAULT_RESERVATION_BYTES: usize = 8 * 1024 * 1024 * 1024;
// Keep the previous default where an 8 GiB virtual reservation is unavailable or costly.
#[cfg(not(all(target_pointer_width = "64", any(unix, windows))))]
const DEFAULT_RESERVATION_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Slot {
    chunk: usize,
    index: usize,
    generation: u64,
}

#[derive(Clone, Copy)]
struct FreeSlot {
    index: usize,
    generation: u64,
}

struct Chunk<T> {
    identity: Arc<()>,
    region: Option<Region>,
    base: NonNull<T>,
    prefix: usize,
    stride: usize,
    capacity: usize,
    next: usize,
    free: Vec<FreeSlot>,
    _marker: PhantomData<T>,
}

impl<T> Chunk<T> {
    fn new(capacity: usize) -> std::io::Result<Self> {
        let stride = size_of::<T>().max(1);
        let alignment = align_of::<T>();
        if size_of::<T>() == 0 {
            return Ok(Self {
                identity: Arc::new(()),
                region: None,
                base: NonNull::dangling(),
                prefix: 0,
                stride,
                capacity,
                next: 0,
                free: Vec::new(),
                _marker: PhantomData,
            });
        }

        let bytes = capacity
            .checked_mul(stride)
            .and_then(|bytes| bytes.checked_add(alignment - 1))
            .ok_or_else(|| std::io::Error::other("StableVec capacity overflow"))?;
        let region = Region::reserve(bytes)?;
        let region_base = region.base();
        let prefix = region_base.as_ptr().align_offset(alignment);
        if prefix == usize::MAX || prefix > alignment - 1 {
            return Err(std::io::Error::other("StableVec alignment overflow"));
        }
        // SAFETY: `bytes` includes `alignment - 1` padding and the checked slot extent;
        // `prefix` stays within that padding, so this retains the region's pointer provenance.
        let aligned = unsafe { region_base.as_ptr().add(prefix).cast::<T>() };
        let base = NonNull::new(aligned)
            .ok_or_else(|| std::io::Error::other("StableVec region returned a null pointer"))?;

        Ok(Self {
            identity: Arc::new(()),
            region: Some(region),
            base,
            prefix,
            stride,
            capacity,
            next: 0,
            free: Vec::new(),
            _marker: PhantomData,
        })
    }

    fn available(&self) -> usize {
        self.free.len().saturating_add(self.capacity - self.next)
    }

    fn prepare(&mut self, additional: usize) -> std::io::Result<()> {
        if additional > self.available() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "StableVec capacity exceeded",
            ));
        }
        let new_slots = additional.saturating_sub(self.free.len());
        if new_slots == 0 {
            return Ok(());
        }
        let Some(region) = &mut self.region else {
            return Ok(());
        };
        let end = self
            .prefix
            .checked_add(
                (self.next + new_slots)
                    .checked_mul(self.stride)
                    .ok_or_else(|| std::io::Error::other("StableVec capacity overflow"))?,
            )
            .ok_or_else(|| std::io::Error::other("StableVec capacity overflow"))?;
        region.commit_to(end)?;
        Ok(())
    }

    fn pointer(&self, index: usize) -> *mut T {
        debug_assert!(index < self.capacity);
        if size_of::<T>() == 0 {
            self.base.as_ptr()
        } else {
            // SAFETY: callers use slots below `capacity`, within this chunk's reserved region.
            unsafe { self.base.as_ptr().add(index) }
        }
    }

    fn allocate(&mut self, value: T) -> std::io::Result<(usize, u64)> {
        self.prepare(1)?;
        let (index, generation) = if let Some(slot) = self.free.pop() {
            (slot.index, slot.generation)
        } else {
            let index = self.next;
            self.next += 1;
            (index, 0)
        };
        // SAFETY: `prepare` commits this slot, and `free.pop`/`next` selects an uninitialized slot.
        unsafe { self.pointer(index).write(value) };
        Ok((index, generation))
    }

    fn reserve_free_slot(&mut self) {
        self.free
            .try_reserve(1)
            .expect("StableVec free-slot allocation failed");
    }

    fn take(&mut self, index: usize, generation: u64) -> T {
        // SAFETY: callers remove this initialized slot from the order table before taking it.
        let value = unsafe { self.pointer(index).read() };
        if let Some(generation) = generation.checked_add(1) {
            self.free.push(FreeSlot { index, generation });
        }
        value
    }
}

/// Non-owning identity for one live element in a [`StableVec`].
///
/// Resolve handles through [`StableVec::get_by_handle`] or
/// [`StableVec::get_mut_by_handle`] to obtain a borrow valid only for that call's borrow
/// of the vector. Handles survive reordering and follow their storage through `append`,
/// but become invalid when their element is removed; cloned vectors have independent handles.
#[derive(Clone, Debug)]
pub struct StableVecHandle {
    chunk: Weak<()>,
    index: usize,
    generation: u64,
}

struct StableVecCore<T> {
    chunks: Vec<Chunk<T>>,
    order: Vec<Slot>,
    capacity: usize,
}

fn prepare_additional<T>(core: &mut StableVecCore<T>, additional: usize) -> std::io::Result<()> {
    if additional > core.capacity - core.order.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            "StableVec capacity exceeded",
        ));
    }
    let mut remaining = additional;
    for chunk in &mut core.chunks {
        let count = remaining.min(chunk.available());
        chunk.prepare(count)?;
        remaining -= count;
        if remaining == 0 {
            break;
        }
    }
    if remaining != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            "StableVec capacity exceeded",
        ));
    }
    Ok(())
}

fn allocate_slot<T>(core: &mut StableVecCore<T>, value: T) -> std::io::Result<Slot> {
    for (chunk_index, chunk) in core.chunks.iter_mut().enumerate() {
        if chunk.available() != 0 {
            let (index, generation) = chunk.allocate(value)?;
            return Ok(Slot {
                chunk: chunk_index,
                index,
                generation,
            });
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::OutOfMemory,
        "StableVec capacity exceeded",
    ))
}

/// Slot-backed storage whose surviving elements keep their addresses across pushes, growth,
/// insertion, removal of other elements, and append. It is not contiguous; use indexing or
/// iteration rather than slice/Deref APIs. References returned by `get` remain valid across
/// push, growth, insertion, and appending into this vector. Removal and mutable access still
/// require an exclusive borrow, so they cannot invalidate a live reference.
pub struct StableVec<T> {
    core: RefCell<StableVecCore<T>>,
}

pub struct Iter<'a, T> {
    vec: &'a StableVec<T>,
    front: usize,
    back: usize,
}

pub struct IterMut<'a, T> {
    vec: *mut StableVec<T>,
    front: usize,
    back: usize,
    _marker: PhantomData<&'a mut StableVec<T>>,
}

impl<T> IterMut<'_, T> {
    fn element_pointer(&self, index: usize) -> Option<*mut T> {
        // SAFETY: the iterator's exclusive lifetime prevents structural edits; these fields are
        // disjoint from element storage, which is reached only through raw pointers in `Chunk`.
        let vec = unsafe { &mut *self.vec };
        let core = vec.core.get_mut();
        let order = &core.order;
        let slot = *order.get(index)?;
        // SAFETY: same iterator invariant; the selected slot is occupied and chunk metadata stays fixed.
        Some(core.chunks.get(slot.chunk)?.pointer(slot.index))
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        self.vec.get(index)
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        let pointer = self.element_pointer(index)?;
        // SAFETY: the iterator advances its front/back bounds, so each unique slot is yielded once.
        Some(unsafe { &mut *pointer })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.back - self.front;
        (len, Some(len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.vec.get(self.back)
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        let pointer = self.element_pointer(self.back)?;
        // SAFETY: the iterator advances its front/back bounds, so each unique slot is yielded once.
        Some(unsafe { &mut *pointer })
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.back - self.front
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.back - self.front
    }
}

impl<T> StableVec<T> {
    pub fn new() -> Self {
        Self::try_new().expect("Failed reserving StableVec region")
    }

    /// Tries to reserve the default address-space capacity, or maximal logical capacity for ZSTs.
    pub fn try_new() -> std::io::Result<Self> {
        let element_size = size_of::<T>();
        let capacity = if element_size == 0 {
            usize::MAX
        } else {
            DEFAULT_RESERVATION_BYTES / element_size
        };
        Self::try_with_capacity(capacity)
    }

    pub fn with_capacity(capacity: usize) -> Option<Self> {
        Self::try_with_capacity(capacity).ok()
    }

    /// Tries to reserve capacity and returns the underlying allocation error.
    pub fn try_with_capacity(capacity: usize) -> std::io::Result<Self> {
        let mut chunks = Vec::new();
        if capacity != 0 {
            chunks
                .try_reserve(1)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            chunks.push(Chunk::new(capacity)?);
        }
        Ok(Self {
            core: RefCell::new(StableVecCore {
                chunks,
                order: Vec::new(),
                capacity,
            }),
        })
    }

    /// Adds reserved slots without relocating any existing elements.
    pub fn grow_to(&self, capacity: usize) -> std::io::Result<()> {
        let mut core = self.core.borrow_mut();
        if capacity <= core.capacity {
            return Ok(());
        }
        let additional = capacity - core.capacity;
        core.chunks
            .try_reserve(1)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let chunk = Chunk::new(additional)?;
        core.chunks.push(chunk);
        core.capacity = capacity;
        Ok(())
    }

    pub fn push(&self, value: T) -> std::io::Result<()> {
        let mut core = self.core.borrow_mut();
        core.order
            .try_reserve(1)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        prepare_additional(&mut core, 1)?;
        let slot = allocate_slot(&mut core, value)?;
        core.order.push(slot);
        Ok(())
    }

    /// Ensures the next push can commit a slot without moving existing elements.
    pub fn prepare_push(&self) -> std::io::Result<()> {
        self.prepare_additional(1)
    }

    /// Commits slots for `additional` elements without moving live elements.
    pub fn prepare_additional(&self, additional: usize) -> std::io::Result<()> {
        prepare_additional(&mut self.core.borrow_mut(), additional)
    }

    /// Compatibility alias; reservation never relocates stable elements.
    pub fn prepare_additional_relocating(&self, additional: usize) -> std::io::Result<()> {
        self.prepare_additional(additional)
    }

    pub fn pop(&mut self) -> Option<T> {
        self.pop_at(self.len().checked_sub(1)?)
    }

    pub fn len(&self) -> usize {
        self.core.borrow().order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.core.borrow().capacity
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let core = self.core.borrow();
        let slot = *core.order.get(index)?;
        let pointer = core.chunks[slot.chunk].pointer(slot.index);
        // SAFETY: slots never move; shared structural edits only touch metadata and uninitialized slots.
        Some(unsafe { &*pointer })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let core = self.core.get_mut();
        let slot = *core.order.get(index)?;
        // SAFETY: the order entry names one initialized slot; `&mut self` makes this borrow unique.
        Some(unsafe { &mut *core.chunks[slot.chunk].pointer(slot.index) })
    }

    /// Creates a non-owning handle for the element at `index`.
    pub fn get_handle(&self, index: usize) -> Option<StableVecHandle> {
        let core = self.core.borrow();
        let slot = *core.order.get(index)?;
        Some(StableVecHandle {
            chunk: Arc::downgrade(&core.chunks[slot.chunk].identity),
            index: slot.index,
            generation: slot.generation,
        })
    }

    /// Resolves a live handle to a temporary shared borrow. Lookup is linear in the number of
    /// elements because logical order is stored separately from address-stable slots.
    pub fn get_by_handle(&self, handle: &StableVecHandle) -> Option<&T> {
        let index = self.index_for_handle(handle)?;
        self.get(index)
    }

    /// Resolves a live handle to a temporary exclusive borrow. Lookup is linear in the number
    /// of elements; Rust borrowing prevents the returned reference from outliving this borrow.
    pub fn get_mut_by_handle(&mut self, handle: &StableVecHandle) -> Option<&mut T> {
        let index = self.index_for_handle(handle)?;
        self.get_mut(index)
    }

    fn index_for_handle(&self, handle: &StableVecHandle) -> Option<usize> {
        let identity = handle.chunk.upgrade()?;
        let core = self.core.borrow();
        let chunk = core
            .chunks
            .iter()
            .position(|candidate| Arc::ptr_eq(&candidate.identity, &identity))?;
        core.order.iter().position(|slot| {
            slot.chunk == chunk
                && slot.index == handle.index
                && slot.generation == handle.generation
        })
    }

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }

    pub fn last(&self) -> Option<&T> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.len()
            .checked_sub(1)
            .and_then(|index| self.get_mut(index))
    }

    pub fn clear(&mut self) {
        while let Some(value) = self.pop() {
            drop(value);
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            vec: self,
            front: 0,
            back: self.len(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let back = self.len();
        IterMut {
            vec: self,
            front: 0,
            back,
            _marker: PhantomData,
        }
    }

    fn pop_at(&mut self, index: usize) -> Option<T> {
        let core = self.core.get_mut();
        let slot = *core.order.get(index)?;
        core.chunks[slot.chunk].reserve_free_slot();
        let slot = core.order.remove(index);
        Some(core.chunks[slot.chunk].take(slot.index, slot.generation))
    }

    /// Inserts in order without moving addresses of existing elements.
    pub fn insert(&self, index: usize, value: T) -> std::io::Result<()> {
        let mut core = self.core.borrow_mut();
        assert!(index <= core.order.len());
        core.order
            .try_reserve(1)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        prepare_additional(&mut core, 1)?;
        let slot = allocate_slot(&mut core, value)?;
        core.order.insert(index, slot);
        Ok(())
    }

    /// Removes an element while preserving order and the addresses of remaining elements.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        (index < self.len()).then(|| self.pop_at(index)).flatten()
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.remove(0)
    }

    /// Removes the suffix without moving addresses of retained elements. Returns false if
    /// `new_len` exceeds the current length.
    pub fn truncate(&mut self, new_len: usize) -> bool {
        if new_len > self.len() {
            return false;
        }

        while self.len() > new_len {
            drop(self.pop());
        }

        true
    }

    /// Removes an element by replacing its logical slot with the last; surviving values keep
    /// their addresses. Returns `None` for an invalid index.
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        let core = self.core.get_mut();
        if index >= core.order.len() {
            return None;
        }
        let removed = core.order[index];
        core.chunks[removed.chunk].reserve_free_slot();
        let last = core.order.pop()?;
        if index == core.order.len() {
            return Some(core.chunks[last.chunk].take(last.index, last.generation));
        }
        let removed = std::mem::replace(&mut core.order[index], last);
        Some(core.chunks[removed.chunk].take(removed.index, removed.generation))
    }

    /// Pre-reserves metadata so a subsequent append cannot allocate after moving elements.
    pub fn reserve_append(&self, other: &Self) -> std::io::Result<()> {
        if std::ptr::eq(self, other) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cannot append a StableVec to itself",
            ));
        }
        let other_core = other.core.borrow();
        let mut core = self.core.borrow_mut();
        core.capacity
            .checked_add(other_core.capacity)
            .ok_or_else(|| std::io::Error::other("StableVec capacity overflow"))?;
        core.order
            .try_reserve(other_core.order.len())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        core.chunks
            .try_reserve(other_core.chunks.len())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(())
    }

    /// Appends another vector by transferring its storage chunks without moving elements.
    /// References into `self` remain valid; `other` is exclusively borrowed during transfer.
    pub fn append(&self, other: &mut Self) {
        self.reserve_append(other)
            .expect("StableVec append allocation failed");
        let mut core = self.core.borrow_mut();
        let mut other_core = other.core.borrow_mut();
        let capacity = core
            .capacity
            .checked_add(other_core.capacity)
            .expect("StableVec capacity overflow");
        let offset = core.chunks.len();
        for slot in other_core.order.drain(..) {
            core.order.push(Slot {
                chunk: offset + slot.chunk,
                index: slot.index,
                generation: slot.generation,
            });
        }
        core.chunks.append(&mut other_core.chunks);
        core.capacity = capacity;
        other_core.capacity = 0;
    }
}

struct StableVecDropGuard<'a, T> {
    values: &'a mut StableVec<T>,
    next: usize,
}

impl<T> StableVecDropGuard<'_, T> {
    fn drop_remaining(&mut self) {
        let core = self.values.core.get_mut();
        while self.next < core.order.len() {
            let slot = core.order[self.next];
            self.next += 1;
            // SAFETY: the order table contains each initialized slot once; advance first so
            // unwinding resumes at the next slot instead of dropping this value twice.
            unsafe { std::ptr::drop_in_place(core.chunks[slot.chunk].pointer(slot.index)) };
        }
    }
}

impl<T> Drop for StableVecDropGuard<'_, T> {
    fn drop(&mut self) {
        self.drop_remaining();
    }
}

impl<T> Drop for StableVec<T> {
    fn drop(&mut self) {
        let mut guard = StableVecDropGuard {
            values: self,
            next: 0,
        };
        guard.drop_remaining();
    }
}

impl<T> Index<usize> for StableVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len() {
            panic!("Program terminated: StableVec out of bounds");
        }
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<usize> for StableVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        if index >= self.len() {
            panic!("Program terminated: StableVec out of bounds");
        }
        self.get_mut(index).unwrap()
    }
}

impl<T> Default for StableVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Extend<T> for StableVec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.push(item).expect("StableVec capacity exceeded");
        }
    }
}

impl<T> FromIterator<T> for StableVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut result = Self::new();

        result.extend(iter);

        result
    }
}

impl<'a, T> IntoIterator for &'a StableVec<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut StableVec<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> IntoIterator for StableVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(mut self) -> Self::IntoIter {
        let mut result = std::vec::Vec::with_capacity(self.len());

        while let Some(item) = self.pop_front() {
            result.push(item);
        }

        result.into_iter()
    }
}

impl<'a, T> std::iter::FusedIterator for Iter<'a, T> {}

impl<'a, T> std::iter::FusedIterator for IterMut<'a, T> {}

impl<T, const N: usize> From<[T; N]> for StableVec<T> {
    fn from(array: [T; N]) -> Self {
        array.into_iter().collect()
    }
}

impl<T: Clone> Clone for StableVec<T> {
    fn clone(&self) -> Self {
        let result = Self::with_capacity(self.capacity()).expect("Failed to allocate StableVec");

        for item in self.iter() {
            result
                .push(item.clone())
                .expect("StableVec capacity exceeded");
        }

        result
    }
}

impl<T: PartialEq> PartialEq for StableVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: Eq> Eq for StableVec<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for StableVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: PartialOrd> PartialOrd for StableVec<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Ord> Ord for StableVec<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<T: std::hash::Hash> std::hash::Hash for StableVec<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for item in self.iter() {
            item.hash(state);
        }
    }
}

impl<T> Add<T> for StableVec<T> {
    type Output = Self;
    fn add(self, rhs: T) -> Self {
        self.push(rhs).expect("StableVec overflow");
        self
    }
}
