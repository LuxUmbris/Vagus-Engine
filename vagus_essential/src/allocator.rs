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

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
use std::io;
use std::ptr::NonNull;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(unix)]
mod virtual_memory {
    use super::*;

    fn page_size() -> io::Result<usize> {
        let page: i64 = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page <= 0 || !(page as usize).is_power_of_two() {
            return Err(io::Error::other("invalid system page size"));
        }
        Ok(page as usize)
    }

    fn round_up(value: usize, alignment: usize) -> Option<usize> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return None;
        }
        value
            .checked_add(alignment - 1)
            .map(|v: usize| v & !(alignment - 1))
    }

    /// Owns a virtual-memory mapping. Cloning an owner would let either copy unmap
    /// storage still referenced by the other.
    ///
    /// ```compile_fail
    /// let region = vagus_essential::allocator::Region::reserve(1).unwrap();
    /// let _other_owner = region.clone();
    /// ```
    #[derive(Debug, PartialEq, Eq)]
    pub struct Region {
        base: NonNull<u8>,
        reserved: usize,
        committed: usize,
    }

    // The mapping is owned and only accessed through the allocator's lock.
    unsafe impl Send for Region {}

    impl Region {
        pub fn reserve(size: usize) -> io::Result<Self> {
            let page = page_size()?;
            let size =
                round_up(size.max(1), page).ok_or_else(|| io::Error::other("size overflow"))?;
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_NONE,
                    libc::MAP_PRIVATE | libc::MAP_ANON,
                    -1,
                    0,
                )
            };

            if ptr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }

            let Some(base) = NonNull::new(ptr.cast()) else {
                unsafe { libc::munmap(ptr, size) };
                return Err(io::Error::other("mmap returned a null address"));
            };

            Ok(Self {
                base,
                reserved: size,
                committed: 0,
            })
        }

        pub fn base(&self) -> NonNull<u8> {
            self.base
        }

        pub fn reserved(&self) -> usize {
            self.reserved
        }

        pub fn committed(&self) -> usize {
            self.committed
        }

        pub fn commit_to(&mut self, end: usize) -> io::Result<()> {
            if end > self.reserved {
                return Err(io::Error::other("region exhausted"));
            }
            if end <= self.committed {
                return Ok(());
            }

            let page = page_size()?;
            let new_end = round_up(end, page).ok_or_else(|| io::Error::other("size overflow"))?;
            if new_end > self.reserved {
                return Err(io::Error::other("region exhausted"));
            }

            let len = new_end - self.committed;
            let ptr = unsafe { self.base.as_ptr().add(self.committed) };
            let result =
                unsafe { libc::mprotect(ptr.cast(), len, libc::PROT_READ | libc::PROT_WRITE) };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }

            self.committed = new_end;
            Ok(())
        }

        #[allow(dead_code)]
        pub fn decommit(&mut self) -> io::Result<()> {
            if self.committed == 0 {
                return Ok(());
            }

            unsafe {
                #[cfg(target_os = "linux")]
                let advice_result = libc::madvise(
                    self.base.as_ptr().cast(),
                    self.committed,
                    libc::MADV_DONTNEED,
                );
                #[cfg(not(target_os = "linux"))]
                let advice_result =
                    libc::madvise(self.base.as_ptr().cast(), self.committed, libc::MADV_FREE);

                if advice_result != 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::mprotect(self.base.as_ptr().cast(), self.committed, libc::PROT_NONE) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }

            self.committed = 0;
            Ok(())
        }
    }

    impl Drop for Region {
        fn drop(&mut self) {
            unsafe { libc::munmap(self.base.as_ptr().cast(), self.reserved) };
        }
    }
}

#[cfg(windows)]
mod virtual_memory {
    use super::*;
    use std::ffi::c_void;

    const MEM_COMMIT: u32 = 0x1000;
    const MEM_DECOMMIT: u32 = 0x4000;
    const MEM_RELEASE: u32 = 0x8000;
    const MEM_RESERVE: u32 = 0x2000;
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_READWRITE: u32 = 0x04;

    #[repr(C)]
    struct SystemInfo {
        oem_id: u32,
        page_size: u32,
        minimum_application_address: *mut c_void,
        maximum_application_address: *mut c_void,
        active_processor_mask: usize,
        number_of_processors: u32,
        processor_type: u32,
        allocation_granularity: u32,
        processor_level: u16,
        processor_revision: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemInfo(info: *mut SystemInfo);
        fn VirtualAlloc(
            address: *mut c_void,
            size: usize,
            allocation_type: u32,
            protect: u32,
        ) -> *mut c_void;
        fn VirtualFree(address: *mut c_void, size: usize, free_type: u32) -> i32;
    }

    fn page_size() -> io::Result<usize> {
        let mut info = std::mem::MaybeUninit::<SystemInfo>::zeroed();
        unsafe { GetSystemInfo(info.as_mut_ptr()) };
        let size = unsafe { info.assume_init() }.page_size as usize;
        if size == 0 || !size.is_power_of_two() {
            return Err(io::Error::other("invalid system page size"));
        }
        Ok(size)
    }

    fn round_up(value: usize, alignment: usize) -> Option<usize> {
        value
            .checked_add(alignment - 1)
            .map(|value| value & !(alignment - 1))
    }

    pub struct Region {
        base: NonNull<u8>,
        reserved: usize,
        committed: usize,
    }

    // The mapping is owned and only accessed through the allocator's lock.
    unsafe impl Send for Region {}

    impl Region {
        pub fn reserve(size: usize) -> io::Result<Self> {
            let page = page_size()?;
            let reserved =
                round_up(size.max(1), page).ok_or_else(|| io::Error::other("size overflow"))?;
            let ptr =
                unsafe { VirtualAlloc(std::ptr::null_mut(), reserved, MEM_RESERVE, PAGE_NOACCESS) };
            let Some(base) = NonNull::new(ptr.cast()) else {
                return Err(io::Error::last_os_error());
            };

            Ok(Self {
                base,
                reserved,
                committed: 0,
            })
        }

        pub fn base(&self) -> NonNull<u8> {
            self.base
        }

        pub fn reserved(&self) -> usize {
            self.reserved
        }

        pub fn committed(&self) -> usize {
            self.committed
        }

        pub fn commit_to(&mut self, end: usize) -> io::Result<()> {
            if end > self.reserved {
                return Err(io::Error::other("region exhausted"));
            }
            if end <= self.committed {
                return Ok(());
            }

            let page = page_size()?;
            let new_end = round_up(end, page).ok_or_else(|| io::Error::other("size overflow"))?;
            if new_end > self.reserved {
                return Err(io::Error::other("region exhausted"));
            }

            let address = unsafe { self.base.as_ptr().add(self.committed) };
            let size = new_end - self.committed;
            let committed =
                unsafe { VirtualAlloc(address.cast(), size, MEM_COMMIT, PAGE_READWRITE) };
            if committed.is_null() {
                return Err(io::Error::last_os_error());
            }
            if committed != address.cast() {
                return Err(io::Error::other(
                    "VirtualAlloc committed an unexpected address",
                ));
            }

            self.committed = new_end;
            Ok(())
        }

        pub fn decommit(&mut self) -> io::Result<()> {
            if self.committed == 0 {
                return Ok(());
            }

            let result =
                unsafe { VirtualFree(self.base.as_ptr().cast(), self.committed, MEM_DECOMMIT) };
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
            self.committed = 0;
            Ok(())
        }
    }

    impl Drop for Region {
        fn drop(&mut self) {
            unsafe { VirtualFree(self.base.as_ptr().cast(), 0, MEM_RELEASE) };
        }
    }
}

#[cfg(any(test, not(any(unix, windows))))]
mod fallback_virtual_memory {
    use super::*;

    pub struct Region {
        base: NonNull<u8>,
        reserved: usize,
        committed: usize,
        layout: Layout,
    }

    // The backing allocation is owned and only accessed through the allocator's lock.
    unsafe impl Send for Region {}

    impl Region {
        pub fn reserve(size: usize) -> io::Result<Self> {
            let size = size.max(1);
            let layout = Layout::from_size_align(size, std::mem::align_of::<usize>())
                .map_err(|_| io::Error::other("invalid region layout"))?;
            let ptr = unsafe { System.alloc(layout) };
            let base = NonNull::new(ptr).ok_or_else(|| io::Error::other("out of memory"))?;
            Ok(Self {
                base,
                reserved: size,
                committed: 0,
                layout,
            })
        }

        pub fn base(&self) -> NonNull<u8> {
            self.base
        }

        pub fn reserved(&self) -> usize {
            self.reserved
        }

        pub fn committed(&self) -> usize {
            self.committed
        }

        pub fn commit_to(&mut self, end: usize) -> io::Result<()> {
            if end > self.reserved {
                return Err(io::Error::other("region exhausted"));
            }
            self.committed = self.committed.max(end);
            Ok(())
        }

        #[allow(dead_code)]
        pub fn decommit(&mut self) -> io::Result<()> {
            // Heap-backed regions cannot release pages while preserving their reservation.
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "heap-backed regions cannot decommit physical memory",
            ))
        }
    }

    impl Drop for Region {
        fn drop(&mut self) {
            unsafe { System.dealloc(self.base.as_ptr(), self.layout) };
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod virtual_memory {
    pub use super::fallback_virtual_memory::Region;
}

pub use virtual_memory::Region;

// Unix and Windows 64-bit backends reserve virtual address space and commit on demand.
#[cfg(all(target_pointer_width = "64", any(unix, windows)))]
const DEFAULT_CHUNK_SIZE: usize = 4 * 1024 * 1024 * 1024;
// Retain the bounded default where large virtual reservations are unavailable or costly.
#[cfg(not(all(target_pointer_width = "64", any(unix, windows))))]
const DEFAULT_CHUNK_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Block {
    offset: usize,
    size: usize,
}

struct Chunk {
    region: Region,
    cursor: usize,
    live_allocations: usize,
    free: Vec<Block>,
}

impl Chunk {
    fn new(size: usize) -> io::Result<Self> {
        Ok(Self {
            region: Region::reserve(size)?,
            cursor: 0,
            live_allocations: 0,
            free: Vec::new(),
        })
    }

    fn allocate(&mut self, layout: Layout) -> io::Result<Option<(NonNull<u8>, Block)>> {
        let free_slots = self
            .live_allocations
            .checked_add(2)
            .ok_or_else(|| io::Error::other("free-list capacity overflow"))?;
        self.free
            .try_reserve(free_slots)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let base = self.region.base().as_ptr() as usize;

        for i in 0..self.free.len() {
            let block = self.free[i];
            let Some(address) = base.checked_add(block.offset) else {
                continue;
            };
            let Some(aligned) = address.checked_add(layout.align() - 1) else {
                continue;
            };
            let aligned = aligned & !(layout.align() - 1);
            let Some(offset) = aligned.checked_sub(base) else {
                continue;
            };
            let Some(end) = offset.checked_add(layout.size()) else {
                continue;
            };
            let Some(block_end) = block.offset.checked_add(block.size) else {
                continue;
            };

            if end <= block_end {
                self.free.remove(i);
                if offset > block.offset {
                    self.insert_free_block(Block {
                        offset: block.offset,
                        size: offset - block.offset,
                    });
                }
                if end < block_end {
                    self.insert_free_block(Block {
                        offset: end,
                        size: block_end - end,
                    });
                }
                let ptr =
                    unsafe { NonNull::new_unchecked(self.region.base().as_ptr().add(offset)) };
                self.live_allocations += 1;
                return Ok(Some((
                    ptr,
                    Block {
                        offset,
                        size: layout.size(),
                    },
                )));
            }
        }

        let address = base
            .checked_add(self.cursor)
            .ok_or_else(|| io::Error::other("address overflow"))?;
        let aligned = address
            .checked_add(layout.align() - 1)
            .ok_or_else(|| io::Error::other("alignment overflow"))?
            & !(layout.align() - 1);
        let offset = aligned
            .checked_sub(base)
            .ok_or_else(|| io::Error::other("address underflow"))?;
        let end = offset
            .checked_add(layout.size())
            .ok_or_else(|| io::Error::other("size overflow"))?;

        if end > self.region.reserved() {
            return Ok(None);
        }

        self.region.commit_to(end)?;
        let block = Block {
            offset: self.cursor,
            size: end - self.cursor,
        };
        self.cursor = end;

        let ptr = unsafe { NonNull::new_unchecked(self.region.base().as_ptr().add(offset)) };
        self.live_allocations += 1;
        Ok(Some((ptr, block)))
    }

    fn deallocate(&mut self, block: Block) {
        debug_assert!(self.live_allocations > 0);
        self.live_allocations -= 1;
        self.insert_free_block(block);
        self.reclaim_if_empty();
    }

    fn insert_free_block(&mut self, mut block: Block) {
        let mut index = self
            .free
            .partition_point(|existing| existing.offset < block.offset);

        if index > 0 {
            let previous = self.free[index - 1];
            if previous.offset.checked_add(previous.size) == Some(block.offset) {
                self.free.remove(index - 1);
                block.offset = previous.offset;
                block.size += previous.size;
                index -= 1;
            }
        }

        if let Some(next) = self.free.get(index).copied() {
            if block.offset.checked_add(block.size) == Some(next.offset) {
                self.free.remove(index);
                block.size += next.size;
            }
        }

        self.free.insert(index, block);
        if let Some(tail) = self.free.last().copied() {
            if tail.offset.checked_add(tail.size) == Some(self.cursor) {
                self.free.pop();
                self.cursor = tail.offset;
            }
        }
    }

    fn reclaim_if_empty(&mut self) {
        let fully_free = self.cursor == 0
            || (self.free.len() == 1
                && self.free[0].offset == 0
                && self.free[0].size >= self.cursor);
        if fully_free && self.region.committed() > 0 && self.region.decommit().is_ok() {
            self.cursor = 0;
            self.free.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Allocator, Block, Chunk, Region, fallback_virtual_memory};
    use std::alloc::Layout;

    #[test]
    fn region_reservation_commits_and_decommits_on_demand() {
        let mut region = Region::reserve(4096).unwrap();
        assert!(region.reserved() >= 4096);
        assert_eq!(region.committed(), 0);

        region.commit_to(1).unwrap();
        assert!(region.committed() >= 1);
        unsafe { region.base().as_ptr().write(0xA5) };

        #[cfg(any(unix, windows))]
        {
            region.decommit().unwrap();
            assert_eq!(region.committed(), 0);
        }
        #[cfg(not(any(unix, windows)))]
        {
            assert_eq!(
                region.decommit().unwrap_err().kind(),
                std::io::ErrorKind::Unsupported
            );
            assert!(region.committed() > 0);
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn region_can_recommit_after_decommit() {
        let mut region = Region::reserve(4096).unwrap();
        let reserved = region.reserved();
        region.commit_to(1).unwrap();

        let base = region.base();
        unsafe { base.as_ptr().write(0xA5) };
        region.decommit().unwrap();
        assert_eq!(region.committed(), 0);
        assert_eq!(region.reserved(), reserved);

        region.commit_to(1).unwrap();
        assert_eq!(region.reserved(), reserved);
        assert!(region.committed() > 0);
        unsafe {
            base.as_ptr().write(0x5A);
            assert_eq!(base.as_ptr().read(), 0x5A);
        }
    }

    #[test]
    fn region_incremental_commit_preserves_data_and_rejects_overrun() {
        let mut region = Region::reserve(64 * 1024 * 1024).unwrap();
        region.commit_to(1).unwrap();
        let first_end = region.committed();
        let base = region.base().as_ptr();
        unsafe { base.add(first_end - 1).write(0xA5) };

        region.commit_to(first_end + 1).unwrap();
        assert!(region.committed() > first_end);
        unsafe {
            base.add(first_end).write(0x5A);
            assert_eq!(base.add(first_end - 1).read(), 0xA5);
        }

        let committed = region.committed();
        assert!(region.commit_to(region.reserved() + 1).is_err());
        assert_eq!(region.committed(), committed);
    }

    #[test]
    fn fallback_region_does_not_claim_physical_decommit_support() {
        let mut region = fallback_virtual_memory::Region::reserve(4096).unwrap();
        assert!(region.reserved() >= 4096);
        region.commit_to(32).unwrap();
        unsafe { region.base().as_ptr().write(0xA5) };
        let committed = region.committed();

        let error = region.decommit().unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(region.committed(), committed);
    }

    #[test]
    fn default_allocator_uses_large_virtual_chunks_on_supported_targets() {
        let mut allocator = Allocator::with_default_size();
        let ptr = allocator
            .allocate_unsafe(Layout::from_size_align(1, 1).unwrap())
            .unwrap();
        let expected_reserved: u64 = if cfg!(all(target_pointer_width = "64", any(unix, windows))) {
            4u64 * 1024 * 1024 * 1024
        } else {
            64u64 * 1024 * 1024
        };

        assert_eq!(allocator.reserved_bytes() as u64, expected_reserved);
        assert!(allocator.committed_bytes() > 0);
        assert!(allocator.committed_bytes() < allocator.reserved_bytes());
        unsafe { allocator.deallocate_unsafe(ptr) };
    }

    #[test]
    fn empty_chunk_decommits_and_reuses_its_reservation() {
        let mut allocator = Allocator::new(64 * 1024);
        let layout = Layout::from_size_align(4096, 1).unwrap();
        let first = allocator.allocate_unsafe(layout).unwrap();
        let reserved = allocator.reserved_bytes();

        assert!(allocator.committed_bytes() > 0);
        assert!(unsafe { allocator.deallocate_unsafe(first) });
        assert_eq!(allocator.reserved_bytes(), reserved);
        #[cfg(any(unix, windows))]
        assert_eq!(allocator.committed_bytes(), 0);
        #[cfg(not(any(unix, windows)))]
        assert!(allocator.committed_bytes() > 0);

        let second = allocator.allocate_unsafe(layout).unwrap();
        assert_eq!(allocator.chunk_count(), 1);
        assert!(allocator.committed_bytes() > 0);
        unsafe { allocator.deallocate_unsafe(second) };
    }

    #[test]
    fn reusing_a_free_block_preserves_its_remainder() {
        let mut chunk = Chunk::new(1024).unwrap();
        let whole = chunk
            .allocate(Layout::from_size_align(256, 1).unwrap())
            .unwrap()
            .unwrap();
        chunk.free.push(whole.1);

        let (first, first_block) = chunk
            .allocate(Layout::from_size_align(64, 1).unwrap())
            .unwrap()
            .unwrap();
        let (second, second_block) = chunk
            .allocate(Layout::from_size_align(192, 1).unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(first_block.size, 64);
        assert_eq!(second_block.size, 192);
        assert_eq!(first.as_ptr() as usize + 64, second.as_ptr() as usize);
        assert!(chunk.free.is_empty());
    }

    #[test]
    fn adjacent_freed_blocks_are_coalesced() {
        let mut chunk = Chunk::new(1024).unwrap();
        chunk.insert_free_block(Block {
            offset: 64,
            size: 64,
        });
        chunk.insert_free_block(Block {
            offset: 0,
            size: 64,
        });

        assert_eq!(chunk.free.len(), 1);
        assert_eq!(chunk.free[0].offset, 0);
        assert_eq!(chunk.free[0].size, 128);
    }

    #[test]
    fn coalescing_with_both_neighbors_recovers_a_larger_extent() {
        let mut chunk = Chunk::new(1024).unwrap();
        let layout = Layout::from_size_align(64, 1).unwrap();
        let (_, _) = chunk.allocate(layout).unwrap().unwrap();
        let (_, left) = chunk.allocate(layout).unwrap().unwrap();
        let (_, middle) = chunk.allocate(layout).unwrap().unwrap();
        let (_, right) = chunk.allocate(layout).unwrap().unwrap();
        let (_, _) = chunk.allocate(layout).unwrap().unwrap();

        chunk.insert_free_block(left);
        chunk.insert_free_block(right);
        chunk.insert_free_block(middle);

        assert_eq!(chunk.free.len(), 1);
        assert_eq!(chunk.free[0].offset, 64);
        assert_eq!(chunk.free[0].size, 192);

        let (reused, block) = chunk
            .allocate(Layout::from_size_align(192, 1).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(reused.as_ptr(), unsafe {
            chunk.region.base().as_ptr().add(64)
        });
        assert_eq!(block.offset, 64);
        assert_eq!(block.size, 192);
        assert!(chunk.free.is_empty());
    }

    #[test]
    fn splitting_a_free_block_preserves_free_list_order() {
        let mut chunk = Chunk::new(1024).unwrap();
        let allocation_layout = Layout::from_size_align(64, 8).unwrap();
        let (_, first) = chunk.allocate(allocation_layout).unwrap().unwrap();
        let (_, _) = chunk.allocate(allocation_layout).unwrap().unwrap();
        let (_, third) = chunk.allocate(allocation_layout).unwrap().unwrap();
        chunk.insert_free_block(first);
        chunk.insert_free_block(third);

        chunk
            .allocate(Layout::from_size_align(32, 8).unwrap())
            .unwrap()
            .unwrap();

        assert!(
            chunk
                .free
                .windows(2)
                .all(|pair| pair[0].offset <= pair[1].offset)
        );
    }

    #[test]
    fn aligned_reuse_preserves_prefix_and_suffix_fragments() {
        let mut chunk = Chunk::new(1024).unwrap();
        chunk
            .allocate(Layout::from_size_align(1, 1).unwrap())
            .unwrap()
            .unwrap();
        let (_, original) = chunk
            .allocate(Layout::from_size_align(64, 1).unwrap())
            .unwrap()
            .unwrap();
        chunk
            .allocate(Layout::from_size_align(1, 1).unwrap())
            .unwrap()
            .unwrap();
        chunk.insert_free_block(original);

        let base = chunk.region.base().as_ptr() as usize;
        let aligned_offset = (base + original.offset + 15) & !15;
        let aligned_offset = aligned_offset - base;
        let prefix_size = aligned_offset - original.offset;
        let suffix_offset = aligned_offset + 32;
        let suffix_size = original.offset + original.size - suffix_offset;

        let (aligned_ptr, aligned_block) = chunk
            .allocate(Layout::from_size_align(32, 16).unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(aligned_ptr.as_ptr() as usize, base + aligned_offset);
        assert_eq!(aligned_block.offset, aligned_offset);
        assert_eq!(aligned_block.size, 32);
        assert_eq!(chunk.free.len(), 2);
        assert_eq!(chunk.free[0].offset, original.offset);
        assert_eq!(chunk.free[0].size, prefix_size);
        assert_eq!(chunk.free[1].offset, suffix_offset);
        assert_eq!(chunk.free[1].size, suffix_size);

        let (_, prefix) = chunk
            .allocate(Layout::from_size_align(prefix_size, 1).unwrap())
            .unwrap()
            .unwrap();
        let (_, suffix) = chunk
            .allocate(Layout::from_size_align(suffix_size, 1).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(prefix.offset, original.offset);
        assert_eq!(suffix.offset, suffix_offset);
        assert!(chunk.free.is_empty());
    }

    #[test]
    fn freeing_the_tail_rewinds_the_bump_cursor() {
        let mut chunk = Chunk::new(4096).unwrap();
        let layout = Layout::from_size_align(2000, 1).unwrap();
        let (_, _) = chunk.allocate(layout).unwrap().unwrap();
        let (_, tail) = chunk.allocate(layout).unwrap().unwrap();
        chunk.insert_free_block(tail);

        let (_, larger) = chunk
            .allocate(Layout::from_size_align(2096, 1).unwrap())
            .unwrap()
            .expect("rewound tail space should fit the larger allocation");

        assert_eq!(larger.offset, 2000);
        assert_eq!(larger.size, 2096);
        assert_eq!(chunk.cursor, 4096);
    }

    #[test]
    fn coalescing_retains_free_list_capacity() {
        let mut chunk = Chunk::new(1024).unwrap();
        chunk.free.reserve(16);
        let capacity = chunk.free.capacity();
        chunk.insert_free_block(Block {
            offset: 64,
            size: 64,
        });
        chunk.insert_free_block(Block {
            offset: 0,
            size: 64,
        });

        assert_eq!(chunk.free.len(), 1);
        assert!(chunk.free.capacity() >= capacity);
    }

    #[test]
    fn allocated_blocks_reserve_free_list_capacity_for_later_deallocation() {
        let mut chunk = Chunk::new(4096).unwrap();
        let layout = Layout::from_size_align(64, 8).unwrap();
        let blocks = (0..16)
            .map(|_| chunk.allocate(layout).unwrap().unwrap().1)
            .collect::<Vec<_>>();
        let capacity = chunk.free.capacity();

        assert!(
            capacity >= blocks.len(),
            "each live allocation needs free-list capacity before it is returned"
        );

        for block in blocks {
            chunk.deallocate(block);
        }

        assert_eq!(chunk.free.capacity(), capacity);
    }
}

struct Allocation {
    chunk: usize,
    block: Block,
}

pub struct Allocator {
    chunks: Vec<Chunk>,
    allocations: HashMap<usize, Allocation>,
    chunk_size: usize,
}

impl Allocator {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunks: Vec::new(),
            allocations: HashMap::new(),
            chunk_size: chunk_size.max(1),
        }
    }

    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_CHUNK_SIZE)
    }

    pub fn allocate_unsafe(&mut self, layout: Layout) -> io::Result<NonNull<u8>> {
        if layout.size() == 0 {
            return Ok(unsafe { NonNull::new_unchecked(layout.align() as *mut u8) });
        }

        for i in 0..self.chunks.len() {
            if let Some((ptr, block)) = self.chunks[i].allocate(layout)? {
                self.allocations
                    .insert(ptr.as_ptr() as usize, Allocation { chunk: i, block });
                return Ok(ptr);
            }
        }

        let required = layout
            .size()
            .checked_add(layout.align())
            .ok_or_else(|| io::Error::other("size overflow"))?;
        let mut chunk = Chunk::new(self.chunk_size.max(required))?;
        let (ptr, block) = chunk
            .allocate(layout)?
            .ok_or_else(|| io::Error::other("allocation failed"))?;
        let index = self.chunks.len();
        self.chunks.push(chunk);
        self.allocations.insert(
            ptr.as_ptr() as usize,
            Allocation {
                chunk: index,
                block,
            },
        );
        Ok(ptr)
    }

    pub unsafe fn deallocate_unsafe(&mut self, ptr: NonNull<u8>) -> bool {
        if let Some(allocation) = self.allocations.remove(&(ptr.as_ptr() as usize)) {
            self.chunks[allocation.chunk].deallocate(allocation.block);
            true
        } else {
            false
        }
    }

    pub unsafe fn realloc(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> io::Result<NonNull<u8>> {
        if old_layout.size() == 0 {
            return self.allocate_unsafe(new_layout);
        }
        if new_layout.size() == 0 {
            unsafe { self.deallocate_unsafe(ptr) };
            return Ok(unsafe { NonNull::new_unchecked(new_layout.align() as *mut u8) });
        }

        let new_ptr = self.allocate_unsafe(new_layout)?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                ptr.as_ptr(),
                new_ptr.as_ptr(),
                old_layout.size().min(new_layout.size()),
            );
            self.deallocate_unsafe(ptr);
        }
        Ok(new_ptr)
    }

    fn owns_allocation(&self, ptr: NonNull<u8>) -> bool {
        self.allocations.contains_key(&(ptr.as_ptr() as usize))
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn committed_bytes(&self) -> usize {
        self.chunks
            .iter()
            .map(|chunk| chunk.region.committed())
            .sum()
    }

    pub fn reserved_bytes(&self) -> usize {
        self.chunks
            .iter()
            .map(|chunk| chunk.region.reserved())
            .sum()
    }
}

impl Default for Allocator {
    fn default() -> Self {
        Self::with_default_size()
    }
}

thread_local! {
    static IN_GLOBAL_ALLOCATOR: Cell<bool> = const { Cell::new(false) };
}

struct AllocatorCallGuard;

impl AllocatorCallGuard {
    fn enter() -> Option<Self> {
        IN_GLOBAL_ALLOCATOR.with(|active| {
            if active.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for AllocatorCallGuard {
    fn drop(&mut self) {
        IN_GLOBAL_ALLOCATOR.with(|active| active.set(false));
    }
}

struct GlobalAllocator {
    allocator: Mutex<Option<Allocator>>,
    deferred_frees: AtomicPtr<DeferredFree>,
}

#[derive(Clone, Copy)]
struct DeferredFree {
    ptr: usize,
    layout: Layout,
    next: *mut DeferredFree,
}

const GLOBAL_SYSTEM_ALLOCATION_THRESHOLD: usize = 4096;
const _: () = assert!(std::mem::size_of::<DeferredFree>() <= GLOBAL_SYSTEM_ALLOCATION_THRESHOLD);

impl GlobalAllocator {
    const fn new() -> Self {
        Self {
            allocator: Mutex::new(None),
            deferred_frees: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    #[cfg(test)]
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Allocator>> {
        self.allocator
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn defer_free(&self, ptr: NonNull<u8>, layout: Layout) {
        // The allocation is dead and large enough to hold its own queue node.
        let node = ptr.cast::<DeferredFree>().as_ptr();
        let mut head = self.deferred_frees.load(Ordering::Relaxed);
        loop {
            // SAFETY: deferred allocations exceed the threshold, and the compile-time
            // size check guarantees room; unaligned access supports any allocation alignment.
            unsafe {
                node.write_unaligned(DeferredFree {
                    ptr: ptr.as_ptr() as usize,
                    layout,
                    next: head,
                });
            }
            match self.deferred_frees.compare_exchange_weak(
                head,
                node,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => head = actual,
            }
        }
    }

    fn drain_deferred_frees(&self, allocator: &mut Allocator) {
        let mut pending = self
            .deferred_frees
            .swap(std::ptr::null_mut(), Ordering::AcqRel);
        while !pending.is_null() {
            // SAFETY: the swap transfers these published nodes exclusively to this drain.
            let free = unsafe { pending.read_unaligned() };
            pending = free.next;
            let ptr = free.ptr as *mut u8;
            let non_null = unsafe { NonNull::new_unchecked(ptr) };
            if !unsafe { allocator.deallocate_unsafe(non_null) } {
                unsafe { System.dealloc(ptr, free.layout) };
            }
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(_guard) = AllocatorCallGuard::enter() else {
            return unsafe { System.alloc(layout) };
        };
        if layout.size() <= GLOBAL_SYSTEM_ALLOCATION_THRESHOLD {
            return unsafe { System.alloc(layout) };
        }

        let mut state = match self.allocator.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            // Avoid making contended allocations queue behind the pool lock.
            Err(std::sync::TryLockError::WouldBlock) => {
                return unsafe { System.alloc(layout) };
            }
        };
        let allocation = {
            let allocator = state.get_or_insert_with(Allocator::with_default_size);
            self.drain_deferred_frees(allocator);
            allocator.allocate_unsafe(layout)
        };
        match allocation {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => {
                drop(state);
                unsafe { System.alloc(layout) }
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || layout.size() == 0 {
            return;
        }
        let Some(_guard) = AllocatorCallGuard::enter() else {
            unsafe { System.dealloc(ptr, layout) };
            return;
        };
        if layout.size() <= GLOBAL_SYSTEM_ALLOCATION_THRESHOLD {
            unsafe { System.dealloc(ptr, layout) };
            return;
        }

        let Some(non_null) = NonNull::new(ptr) else {
            return;
        };
        let mut state = match self.allocator.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                self.defer_free(non_null, layout);
                return;
            }
        };
        let handled = state.as_mut().is_some_and(|allocator| {
            self.drain_deferred_frees(allocator);
            unsafe { allocator.deallocate_unsafe(non_null) }
        });
        if !handled {
            unsafe { System.dealloc(ptr, layout) };
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        let Some(_guard) = AllocatorCallGuard::enter() else {
            return unsafe { System.realloc(ptr, old_layout, new_size) };
        };

        let Some(non_null) = NonNull::new(ptr) else {
            return std::ptr::null_mut();
        };
        let Ok(new_layout) = Layout::from_size_align(new_size, old_layout.align()) else {
            return std::ptr::null_mut();
        };

        if old_layout.size() <= GLOBAL_SYSTEM_ALLOCATION_THRESHOLD {
            if new_size <= GLOBAL_SYSTEM_ALLOCATION_THRESHOLD {
                return unsafe { System.realloc(ptr, old_layout, new_size) };
            }

            let mut state = match self.allocator.try_lock() {
                Ok(state) => state,
                Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    return unsafe { System.realloc(ptr, old_layout, new_size) };
                }
            };
            let allocator = state.get_or_insert_with(Allocator::with_default_size);
            self.drain_deferred_frees(allocator);
            let allocation = allocator.allocate_unsafe(new_layout);
            let new_ptr = match allocation {
                Ok(new_ptr) => new_ptr,
                Err(_) => {
                    drop(state);
                    let new_ptr = unsafe { System.alloc(new_layout) };
                    if new_ptr.is_null() {
                        return std::ptr::null_mut();
                    }
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            ptr,
                            new_ptr,
                            old_layout.size().min(new_size),
                        );
                        System.dealloc(ptr, old_layout);
                    }
                    return new_ptr;
                }
            };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    ptr,
                    new_ptr.as_ptr(),
                    old_layout.size().min(new_size),
                );
                System.dealloc(ptr, old_layout);
            }
            return new_ptr.as_ptr();
        }

        let mut state = match self.allocator.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                let new_ptr = unsafe { System.alloc(new_layout) };
                if new_ptr.is_null() {
                    return std::ptr::null_mut();
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(ptr, new_ptr, old_layout.size().min(new_size));
                }
                self.defer_free(non_null, old_layout);
                return new_ptr;
            }
        };
        let Some(allocator) = state.as_mut() else {
            drop(state);
            return unsafe { System.realloc(ptr, old_layout, new_size) };
        };
        self.drain_deferred_frees(allocator);
        if !allocator.owns_allocation(non_null) {
            drop(state);
            return unsafe { System.realloc(ptr, old_layout, new_size) };
        }

        if new_size <= GLOBAL_SYSTEM_ALLOCATION_THRESHOLD {
            let new_ptr = unsafe { System.alloc(new_layout) };
            if new_ptr.is_null() {
                return std::ptr::null_mut();
            }
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, new_ptr, old_layout.size().min(new_size));
                allocator.deallocate_unsafe(non_null);
            }
            return new_ptr;
        }

        match unsafe { allocator.realloc(non_null, old_layout, new_layout) } {
            Ok(new_ptr) => new_ptr.as_ptr(),
            Err(_) => {
                let new_ptr = unsafe { System.alloc(new_layout) };
                if new_ptr.is_null() {
                    return std::ptr::null_mut();
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        ptr,
                        new_ptr,
                        old_layout.size().min(new_size),
                    );
                    allocator.deallocate_unsafe(non_null);
                }
                new_ptr
            }
        }
    }
}

#[global_allocator]
static GLOBAL: GlobalAllocator = GlobalAllocator::new();

#[cfg(test)]
mod global_allocator_tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    #[test]
    fn large_allocations_fall_back_to_system_when_pool_reservation_fails() {
        let global = GlobalAllocator::new();
        *global.lock() = Some(Allocator::new(usize::MAX));
        let layout = Layout::from_size_align(8192, 8).unwrap();

        let ptr = unsafe { global.alloc(layout) };

        assert!(
            !ptr.is_null(),
            "system fallback should satisfy the allocation"
        );
        unsafe {
            std::ptr::write_bytes(ptr, 0xA5, layout.size());
            assert!(std::slice::from_raw_parts(ptr, layout.size())
                .iter()
                .all(|byte| *byte == 0xA5));
            global.dealloc(ptr, layout);
        }
    }

    #[test]
    fn page_sized_allocations_bypass_the_shared_pool() {
        let global = GlobalAllocator::new();
        let layout = Layout::from_size_align(4096, 8).unwrap();

        let ptr = unsafe { global.alloc(layout) };
        assert!(!ptr.is_null());
        let pool_was_uninitialized = global.lock().is_none();
        unsafe {
            std::ptr::write_bytes(ptr, 0xA5, layout.size());
            global.dealloc(ptr, layout);
        }

        assert!(
            pool_was_uninitialized,
            "4 KiB allocations should use System"
        );
        assert!(
            global.lock().is_none(),
            "freeing must not initialize the pool"
        );
    }

    #[test]
    fn small_allocations_do_not_wait_for_the_allocator_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let guard = global.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);

        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = unsafe { worker_global.alloc(layout) };
            if !ptr.is_null() {
                unsafe { worker_global.dealloc(ptr, layout) };
            }
            done_tx.send(!ptr.is_null()).unwrap();
        });

        started_rx.recv().unwrap();
        let completed_while_locked = done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or(false);
        drop(guard);
        worker.join().unwrap();
        assert!(
            completed_while_locked,
            "small allocations should bypass the shared allocator mutex"
        );
    }

    #[test]
    fn large_allocations_do_not_wait_for_the_allocator_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let guard = global.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);
        let layout = Layout::from_size_align(8192, 8).unwrap();

        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let ptr = unsafe { worker_global.alloc(layout) };
            done_tx.send(!ptr.is_null()).unwrap();
            ptr as usize
        });

        started_rx.recv().unwrap();
        let completed_while_locked = done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or(false);
        drop(guard);
        let ptr = worker.join().unwrap() as *mut u8;
        if !ptr.is_null() {
            unsafe { global.dealloc(ptr, layout) };
        }

        assert!(
            completed_while_locked,
            "contended large allocations should use the system allocator"
        );
    }

    #[test]
    fn freeing_a_pool_allocation_does_not_wait_for_the_allocator_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let layout = Layout::from_size_align(8192, 8).unwrap();
        let ptr = unsafe { global.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xA5, layout.size()) };

        let guard = global.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);
        let ptr_addr = ptr as usize;
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            unsafe { worker_global.dealloc(ptr_addr as *mut u8, layout) };
            done_tx.send(()).unwrap();
        });

        started_rx.recv().unwrap();
        let completed_while_locked = done_rx.recv_timeout(Duration::from_millis(100)).is_ok();
        drop(guard);
        worker.join().unwrap();

        assert!(
            completed_while_locked,
            "freeing a pool allocation should not wait for the shared allocator mutex"
        );

        let reused = unsafe { global.alloc(layout) };
        assert_eq!(reused, ptr);
        unsafe { global.dealloc(reused, layout) };
    }

    #[test]
    fn concurrent_deferred_pool_frees_are_reclaimed() {
        const FREE_COUNT: usize = 8;

        let global = Arc::new(GlobalAllocator::new());
        let prefix_layout = Layout::from_size_align(4097, 1).unwrap();
        let prefix = unsafe { global.alloc(prefix_layout) };
        assert!(!prefix.is_null());
        let layout = Layout::from_size_align(8192, 1).unwrap();
        let pointers = (0..FREE_COUNT)
            .map(|_| {
                let ptr = unsafe { global.alloc(layout) };
                assert!(!ptr.is_null());
                assert_ne!(ptr as usize % std::mem::align_of::<DeferredFree>(), 0);
                ptr
            })
            .collect::<Vec<_>>();
        let allocator_guard = global.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let workers = pointers
            .iter()
            .map(|ptr| {
                let worker_global = Arc::clone(&global);
                let ptr = *ptr as usize;
                let started_tx = started_tx.clone();
                let done_tx = done_tx.clone();
                std::thread::spawn(move || {
                    started_tx.send(()).unwrap();
                    unsafe { worker_global.dealloc(ptr as *mut u8, layout) };
                    done_tx.send(()).unwrap();
                })
            })
            .collect::<Vec<_>>();
        drop(started_tx);
        drop(done_tx);

        for _ in 0..FREE_COUNT {
            started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        }
        let completed_while_locked =
            (0..FREE_COUNT).all(|_| done_rx.recv_timeout(Duration::from_secs(1)).is_ok());
        drop(allocator_guard);
        for worker in workers {
            worker.join().unwrap();
        }

        assert!(
            completed_while_locked,
            "concurrent deferred frees should not wait for the pool mutex"
        );

        let mut reused = (0..FREE_COUNT)
            .map(|_| unsafe { global.alloc(layout) } as usize)
            .collect::<Vec<_>>();
        let mut expected = pointers.iter().map(|ptr| *ptr as usize).collect::<Vec<_>>();
        reused.sort_unstable();
        expected.sort_unstable();
        assert_eq!(reused, expected);
        for ptr in reused {
            unsafe { global.dealloc(ptr as *mut u8, layout) };
        }
        unsafe { global.dealloc(prefix, prefix_layout) };
    }

    #[test]
    fn reallocating_a_pool_allocation_does_not_wait_for_the_allocator_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let old_layout = Layout::from_size_align(8192, 8).unwrap();
        let new_layout = Layout::from_size_align(16384, 8).unwrap();
        let ptr = unsafe { global.alloc(old_layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xA5, old_layout.size()) };

        let guard = global.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);
        let ptr_addr = ptr as usize;
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let new_ptr = unsafe {
                worker_global.realloc(ptr_addr as *mut u8, old_layout, new_layout.size())
            };
            done_tx.send(new_ptr as usize).unwrap();
        });

        started_rx.recv().unwrap();
        let result_while_locked = done_rx.recv_timeout(Duration::from_millis(100));
        let completed_while_locked = result_while_locked.is_ok();
        drop(guard);
        let new_ptr = result_while_locked.unwrap_or_else(|_| done_rx.recv().unwrap()) as *mut u8;
        worker.join().unwrap();

        assert!(
            completed_while_locked,
            "reallocating a pool allocation should not wait for the shared allocator mutex"
        );
        assert!(!new_ptr.is_null());
        let preserved = unsafe { std::slice::from_raw_parts(new_ptr, old_layout.size()) };
        assert!(preserved.iter().all(|byte| *byte == 0xA5));

        unsafe { global.dealloc(new_ptr, new_layout) };
        let reused = unsafe { global.alloc(old_layout) };
        assert_eq!(reused, ptr);
        unsafe { global.dealloc(reused, old_layout) };
    }

    #[test]
    fn growing_a_system_allocation_does_not_wait_for_the_allocator_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let old_layout = Layout::from_size_align(64, 8).unwrap();
        let new_layout = Layout::from_size_align(8192, 8).unwrap();
        let ptr = unsafe { global.alloc(old_layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xA5, old_layout.size()) };

        let guard = global.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);
        let ptr_addr = ptr as usize;
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let new_ptr = unsafe {
                worker_global.realloc(ptr_addr as *mut u8, old_layout, new_layout.size())
            };
            done_tx.send(new_ptr as usize).unwrap();
        });

        started_rx.recv().unwrap();
        let result_while_locked = done_rx.recv_timeout(Duration::from_millis(100));
        let completed_while_locked = result_while_locked.is_ok();
        drop(guard);
        let new_ptr = result_while_locked.unwrap_or_else(|_| done_rx.recv().unwrap()) as *mut u8;
        worker.join().unwrap();

        assert!(
            completed_while_locked,
            "growing a system allocation should not wait for the shared allocator mutex"
        );
        assert!(!new_ptr.is_null());
        let preserved = unsafe { std::slice::from_raw_parts(new_ptr, old_layout.size()) };
        assert!(preserved.iter().all(|byte| *byte == 0xA5));
        unsafe { global.dealloc(new_ptr, new_layout) };
    }

    #[test]
    fn freeing_a_large_system_fallback_does_not_wait_for_the_pool_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let guard = global.lock();
        let (allocated_tx, allocated_rx) = mpsc::channel();
        let (free_tx, free_rx) = mpsc::channel();
        let (freed_tx, freed_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);

        let worker = std::thread::spawn(move || {
            let layout = Layout::from_size_align(8192, 8).unwrap();
            let ptr = unsafe { worker_global.alloc(layout) };
            allocated_tx.send(!ptr.is_null()).unwrap();
            free_rx.recv().unwrap();
            if !ptr.is_null() {
                unsafe { worker_global.dealloc(ptr, layout) };
            }
            freed_tx.send(()).unwrap();
        });

        let allocated_while_locked = allocated_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        free_tx.send(()).unwrap();
        let freed_while_locked = freed_rx.recv_timeout(Duration::from_millis(100)).is_ok();
        drop(guard);
        worker.join().unwrap();

        assert!(
            allocated_while_locked,
            "contended allocation should succeed"
        );
        assert!(
            freed_while_locked,
            "freeing a system fallback should bypass the pool mutex"
        );
        let cleanup_layout = Layout::from_size_align(8192, 8).unwrap();
        let cleanup = unsafe { global.alloc(cleanup_layout) };
        assert!(!cleanup.is_null());
        unsafe { global.dealloc(cleanup, cleanup_layout) };
    }

    #[test]
    fn reallocating_a_large_system_fallback_does_not_wait_for_the_pool_mutex() {
        let global = Arc::new(GlobalAllocator::new());
        let guard = global.lock();
        let (allocated_tx, allocated_rx) = mpsc::channel();
        let (realloc_tx, realloc_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_global = Arc::clone(&global);

        let worker = std::thread::spawn(move || {
            let old_layout = Layout::from_size_align(8192, 8).unwrap();
            let new_layout = Layout::from_size_align(16384, 8).unwrap();
            let ptr = unsafe { worker_global.alloc(old_layout) };
            if !ptr.is_null() {
                unsafe { std::ptr::write_bytes(ptr, 0xA5, old_layout.size()) };
            }
            allocated_tx.send(!ptr.is_null()).unwrap();
            realloc_rx.recv().unwrap();

            let new_ptr = unsafe { worker_global.realloc(ptr, old_layout, new_layout.size()) };
            let preserved = !new_ptr.is_null()
                && unsafe {
                    std::slice::from_raw_parts(new_ptr, old_layout.size())
                        .iter()
                        .all(|byte| *byte == 0xA5)
                };
            if !new_ptr.is_null() {
                unsafe { worker_global.dealloc(new_ptr, new_layout) };
            }
            done_tx.send(preserved).unwrap();
        });

        let allocated_while_locked = allocated_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        realloc_tx.send(()).unwrap();
        let completed_while_locked = done_rx
            .recv_timeout(Duration::from_millis(100))
            .unwrap_or(false);
        drop(guard);
        worker.join().unwrap();

        assert!(
            allocated_while_locked,
            "contended allocation should succeed"
        );
        assert!(
            completed_while_locked,
            "reallocating and freeing a system fallback should bypass the pool mutex"
        );
        let cleanup_layout = Layout::from_size_align(8192, 8).unwrap();
        let cleanup = unsafe { global.alloc(cleanup_layout) };
        assert!(!cleanup.is_null());
        unsafe { global.dealloc(cleanup, cleanup_layout) };
    }

    #[test]
    fn reallocations_preserve_bytes_across_system_and_pool_allocations() {
        let global = GlobalAllocator::new();
        let small_layout = Layout::from_size_align(64, 8).unwrap();
        let large_layout = Layout::from_size_align(8192, 8).unwrap();
        let small_ptr = unsafe { global.alloc(small_layout) };
        assert!(!small_ptr.is_null());
        unsafe { std::ptr::write_bytes(small_ptr, 0xA5, small_layout.size()) };

        let large_ptr = unsafe { global.realloc(small_ptr, small_layout, large_layout.size()) };
        assert!(!large_ptr.is_null());
        let preserved = unsafe { std::slice::from_raw_parts(large_ptr, small_layout.size()) };
        assert!(preserved.iter().all(|byte| *byte == 0xA5));

        let small_ptr = unsafe { global.realloc(large_ptr, large_layout, small_layout.size()) };
        assert!(!small_ptr.is_null());
        let preserved = unsafe { std::slice::from_raw_parts(small_ptr, small_layout.size()) };
        assert!(preserved.iter().all(|byte| *byte == 0xA5));
        unsafe { global.dealloc(small_ptr, small_layout) };
    }

    #[test]
    fn growing_system_allocation_falls_back_when_pool_reservation_fails() {
        let global = GlobalAllocator::new();
        *global.lock() = Some(Allocator::new(usize::MAX));
        let old_layout = Layout::from_size_align(64, 8).unwrap();
        let new_layout = Layout::from_size_align(8192, 8).unwrap();
        let ptr = unsafe { global.alloc(old_layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xA5, old_layout.size()) };

        let new_ptr = unsafe { global.realloc(ptr, old_layout, new_layout.size()) };

        assert!(!new_ptr.is_null(), "system fallback should satisfy realloc");
        let preserved = unsafe { std::slice::from_raw_parts(new_ptr, old_layout.size()) };
        assert!(preserved.iter().all(|byte| *byte == 0xA5));
        unsafe { global.dealloc(new_ptr, new_layout) };
    }

    #[test]
    fn growing_pool_allocation_falls_back_when_next_chunk_reservation_fails() {
        let global = GlobalAllocator::new();
        *global.lock() = Some(Allocator::new(16384));
        let old_layout = Layout::from_size_align(8192, 8).unwrap();
        let new_layout = Layout::from_size_align(16384, 8).unwrap();
        let ptr = unsafe { global.alloc(old_layout) };
        assert!(!ptr.is_null());
        unsafe { std::ptr::write_bytes(ptr, 0xA5, old_layout.size()) };
        {
            let mut state = global.lock();
            let allocator = state.as_mut().unwrap();
            allocator.chunk_size = usize::MAX;
            allocator.chunks[0].cursor = allocator.chunks[0].region.reserved();
        }

        let new_ptr = unsafe { global.realloc(ptr, old_layout, new_layout.size()) };

        assert!(!new_ptr.is_null(), "system fallback should satisfy realloc");
        let preserved = unsafe { std::slice::from_raw_parts(new_ptr, old_layout.size()) };
        assert!(preserved.iter().all(|byte| *byte == 0xA5));
        unsafe { global.dealloc(new_ptr, new_layout) };
    }
}
