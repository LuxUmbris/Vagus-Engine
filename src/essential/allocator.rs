use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
use std::io;
use std::ptr::NonNull;
use std::sync::Mutex;

#[cfg(unix)]
mod virtual_memory {
    use super::*;

    fn page_size() -> io::Result<usize> {
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
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
            .map(|v| v & !(alignment - 1))
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

#[cfg(not(unix))]
mod virtual_memory {
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
            unsafe { std::ptr::write_bytes(self.base.as_ptr(), 0, self.committed) };
            self.committed = 0;
            Ok(())
        }
    }

    impl Drop for Region {
        fn drop(&mut self) {
            unsafe { System.dealloc(self.base.as_ptr(), self.layout) };
        }
    }
}

use virtual_memory::Region;

const DEFAULT_CHUNK_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Block {
    offset: usize,
    size: usize,
}

struct Chunk {
    region: Region,
    cursor: usize,
    free: Vec<Block>,
}

impl Chunk {
    fn new(size: usize) -> io::Result<Self> {
        Ok(Self {
            region: Region::reserve(size)?,
            cursor: 0,
            free: Vec::new(),
        })
    }

    fn allocate(&mut self, layout: Layout) -> io::Result<Option<(NonNull<u8>, Block)>> {
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
                self.free.swap_remove(i);
                let ptr =
                    unsafe { NonNull::new_unchecked(self.region.base().as_ptr().add(offset)) };
                return Ok(Some((ptr, block)));
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
        Ok(Some((ptr, block)))
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

    /// Releases a live allocation made by this allocator.
    ///
    /// # Safety
    /// `ptr` must be a live allocation returned by this allocator and not already freed.
    pub unsafe fn deallocate_unsafe(&mut self, ptr: NonNull<u8>) -> bool {
        if let Some(allocation) = self.allocations.remove(&(ptr.as_ptr() as usize)) {
            self.chunks[allocation.chunk].free.push(allocation.block);
            true
        } else {
            false
        }
    }

    /// Reallocates a live block, preserving its contents up to the smaller size.
    ///
    /// # Safety
    /// `ptr` must be a live allocation returned by this allocator for `old_layout`.
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
}

impl GlobalAllocator {
    const fn new() -> Self {
        Self {
            allocator: Mutex::new(None),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Allocator>> {
        self.allocator
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(_guard) = AllocatorCallGuard::enter() else {
            return unsafe { System.alloc(layout) };
        };

        let mut state = self.lock();
        let allocator = state.get_or_insert_with(Allocator::with_default_size);
        allocator
            .allocate_unsafe(layout)
            .map_or(std::ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || layout.size() == 0 {
            return;
        }
        let Some(_guard) = AllocatorCallGuard::enter() else {
            unsafe { System.dealloc(ptr, layout) };
            return;
        };

        let Some(non_null) = NonNull::new(ptr) else {
            return;
        };
        let handled = {
            let mut state = self.lock();
            state
                .as_mut()
                .is_some_and(|allocator| unsafe { allocator.deallocate_unsafe(non_null) })
        };
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

        let mut state = self.lock();
        let Some(allocator) = state.as_mut() else {
            drop(state);
            return unsafe { System.realloc(ptr, old_layout, new_size) };
        };
        if !allocator.owns_allocation(non_null) {
            drop(state);
            return unsafe { System.realloc(ptr, old_layout, new_size) };
        }

        unsafe { allocator.realloc(non_null, old_layout, new_layout) }
            .map_or(std::ptr::null_mut(), |new_ptr| new_ptr.as_ptr())
    }
}

#[global_allocator]
static GLOBAL: GlobalAllocator = GlobalAllocator::new();
