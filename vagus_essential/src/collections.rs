use super::allocator::Region;
use std::ops::{Add, Index, IndexMut};

pub struct StableVec<T> {
    region: Region,
    base: *mut T,
    head: usize,
    len: usize,
    capacity: usize,
}

pub struct Iter<'a, T> {
    current: *const T,
    remaining: usize,
    _marker: std::marker::PhantomData<&'a T>,
}

pub struct IterMut<'a, T> {
    current: *mut T,
    remaining: usize,
    _marker: std::marker::PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            let item = &*self.current;
            self.current = self.current.add(1);
            self.remaining -= 1;
            Some(item)
        }
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            let item = &mut *self.current;
            self.current = self.current.add(1);
            self.remaining -= 1;
            Some(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            self.remaining -= 1;
            Some(&*self.current.add(self.remaining))
        }
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            self.remaining -= 1;
            Some(&mut *self.current.add(self.remaining))
        }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T> StableVec<T> {
    pub fn new() -> Self {
        let capacity = (1024 * 1024 * 1024) / size_of::<T>().max(1);
        Self::with_capacity(capacity).expect("Failed reserving StableVec region")
    }

    pub fn with_capacity(capacity: usize) -> Option<Self> {
        let alignment = align_of::<T>();
        let bytes = capacity.checked_mul(size_of::<T>().max(1))?;
        let reserved = bytes.checked_add(alignment - 1)?;
        let region = Region::reserve(reserved).ok()?;
        let region_base = region.base().as_ptr() as usize;
        let aligned = region_base.checked_add(alignment - 1)? & !(alignment - 1);
        let base = aligned as *mut T;

        Some(Self {
            region,
            base,
            head: 0,
            len: 0,
            capacity,
        })
    }

    pub fn push(&mut self, value: T) -> std::io::Result<()> {
        self.prepare_push()?;

        let tail = self.head + self.len;
        unsafe {
            self.base.add(tail).write(value);
        }
        self.len += 1;

        Ok(())
    }

    /// Ensures the next push can commit its slot without changing the length.
    pub fn prepare_push(&mut self) -> std::io::Result<()> {
        if self.len == self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "StableVec capacity exceeded",
            ));
        }

        self.ensure_tail();
        let tail = self.head + self.len;
        self.commit_to_tail(tail + 1)
    }

    /// Commits room for `additional` elements without changing the length.
    pub fn prepare_additional(&mut self, additional: usize) -> std::io::Result<()> {
        if additional > self.capacity - self.len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "StableVec capacity exceeded",
            ));
        }
        if additional == 0 {
            return Ok(());
        }

        let tail = self
            .head
            .checked_add(self.len)
            .and_then(|tail| tail.checked_add(additional))
            .ok_or_else(|| std::io::Error::other("StableVec capacity overflow"))?;
        if tail > self.capacity {
            unsafe {
                std::ptr::copy(self.base.add(self.head), self.base, self.len);
            }
            self.head = 0;
        }

        self.commit_to_tail(self.head + self.len + additional)
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        unsafe {
            self.len -= 1;
            let value = self.base.add(self.head + self.len).read();
            if self.len == 0 {
                self.head = 0;
            }
            Some(value)
        }
    }

    fn commit_to_tail(&mut self, tail: usize) -> std::io::Result<()> {
        let prefix = self.base as usize - self.region.base().as_ptr() as usize;
        let bytes = tail
            .checked_mul(size_of::<T>().max(1))
            .and_then(|bytes| prefix.checked_add(bytes))
            .ok_or_else(|| std::io::Error::other("StableVec capacity overflow"))?;
        self.region.commit_to(bytes)
    }

    fn ensure_tail(&mut self) {
        if self.head + self.len == self.capacity && self.head != 0 {
            unsafe {
                std::ptr::copy(self.base.add(self.head), self.base, self.len);
            }
            self.head = 0;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        unsafe { Some(&*self.base.add(self.head + index)) }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }

        unsafe { Some(&mut *self.base.add(self.head + index)) }
    }

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }

    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        unsafe { Some(&*self.base.add(self.head + self.len - 1)) }
    }

    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.len == 0 {
            return None;
        }
        unsafe { Some(&mut *self.base.add(self.head + self.len - 1)) }
    }

    pub fn clear(&mut self) {
        while let Some(value) = self.pop() {
            drop(value);
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            current: unsafe { self.base.add(self.head) },
            remaining: self.len,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            current: unsafe { self.base.add(self.head) },
            remaining: self.len,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn insert(&mut self, index: usize, value: T) -> std::io::Result<()> {
        assert!(index <= self.len());
        if self.len == self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "StableVec capacity exceeded",
            ));
        }

        self.ensure_tail();
        let tail = self.head + self.len;
        self.commit_to_tail(tail + 1)?;

        unsafe {
            let ptr = self.base.add(self.head + index);

            std::ptr::copy(ptr, ptr.add(1), self.len() - index);

            ptr.write(value);
            self.len += 1;
        }
        Ok(())
    }

    /// Removes an element while preserving the order of the remaining elements.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        unsafe {
            let target = self.base.add(self.head + index);
            let removed = target.read();
            std::ptr::copy(target.add(1), target, self.len - index - 1);
            self.len -= 1;
            if self.len == 0 {
                self.head = 0;
            }
            Some(removed)
        }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        unsafe {
            let value = self.base.add(self.head).read();
            self.head += 1;
            self.len -= 1;
            if self.len == 0 {
                self.head = 0;
            }
            Some(value)
        }
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.base.add(self.head), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.base.add(self.head), self.len) }
    }

    /// returns false if the total length is smaller than the index from which should be truncated
    pub fn truncate(&mut self, new_len: usize) -> bool {
        if new_len > self.len() {
            return false;
        }

        while self.len() > new_len {
            drop(self.pop());
        }

        true
    }

    /// returns false if out of bounds
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        unsafe {
            let target = self.base.add(self.head + index);
            let last = self.base.add(self.head + self.len - 1);

            let removed = target.read();

            if index + 1 != self.len {
                target.write(last.read());
            }

            self.len -= 1;
            if self.len == 0 {
                self.head = 0;
            }

            Some(removed)
        }
    }

    pub fn append(&mut self, other: &mut Self) {
        assert!(
            other.len <= self.capacity - self.len,
            "StableVec capacity exceeded"
        );
        while let Some(value) = other.pop_front() {
            self.push(value).expect("StableVec capacity exceeded");
        }
    }
}

impl<T> Drop for StableVec<T> {
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        while self.pop().is_some() {}
        self.region.decommit();
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

impl<T> std::ops::Deref for StableVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> std::ops::DerefMut for StableVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> Default for StableVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AsRef<[T]> for StableVec<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> AsMut<[T]> for StableVec<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> std::borrow::Borrow<[T]> for StableVec<T> {
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> std::borrow::BorrowMut<[T]> for StableVec<T> {
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
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
        let mut result = Self::with_capacity(self.capacity).expect("Failed to allocate StableVec");

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
        self.as_slice() == other.as_slice()
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
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord> Ord for StableVec<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: std::hash::Hash> std::hash::Hash for StableVec<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl<T> Add<T> for StableVec<T> {
    type Output = Self;
    fn add(mut self, rhs: T) -> Self {
        self.push(rhs).expect("StableVec overflow");
        self
    }
}
