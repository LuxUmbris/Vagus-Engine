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

use std::alloc::Layout;
use vagus::essential::allocator::Allocator;

#[test]
fn zero_sized_allocations_respect_layout_alignment() {
    let mut allocator = Allocator::new(4096);
    let layout = Layout::from_size_align(0, 256).unwrap();

    let ptr = allocator.allocate_unsafe(layout).unwrap();

    assert_eq!(ptr.as_ptr() as usize % layout.align(), 0);
}

#[test]
fn freed_blocks_can_be_reused_without_losing_alignment() {
    let mut allocator = Allocator::new(4096);
    let layout = Layout::from_size_align(32, 64).unwrap();

    let first = allocator.allocate_unsafe(layout).unwrap();
    unsafe { allocator.deallocate_unsafe(first) };
    let reused = allocator.allocate_unsafe(layout).unwrap();

    assert_eq!(reused, first);
    assert_eq!(reused.as_ptr() as usize % layout.align(), 0);
}

#[test]
fn realloc_preserves_contents_and_obeys_new_alignment() {
    let mut allocator = Allocator::new(4096);
    let old_layout = Layout::from_size_align(24, 8).unwrap();
    let new_layout = Layout::from_size_align(128, 128).unwrap();
    let old = allocator.allocate_unsafe(old_layout).unwrap();
    unsafe { std::ptr::write_bytes(old.as_ptr(), 0xA5, old_layout.size()) };

    let new = unsafe { allocator.realloc(old, old_layout, new_layout).unwrap() };

    assert_eq!(new.as_ptr() as usize % new_layout.align(), 0);
    let bytes = unsafe { std::slice::from_raw_parts(new.as_ptr(), old_layout.size()) };
    assert!(bytes.iter().all(|byte| *byte == 0xA5));
}

#[test]
fn global_allocator_handles_growth_and_concurrent_allocations() {
    let workers = (0..4)
        .map(|worker| {
            std::thread::spawn(move || {
                (
                    worker,
                    (0..10_000).map(|value| value + worker).collect::<Vec<_>>(),
                )
            })
        })
        .collect::<Vec<_>>();

    for worker in workers {
        let (worker, values) = worker.join().unwrap();
        assert_eq!(values.len(), 10_000);
        assert_eq!(values[0], worker);
        assert_eq!(values[9_999], 9_999 + worker);
    }
}
