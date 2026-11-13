use std::ptr::NonNull;

struct StableVec {
    base: *mut u8,
    start: NonNull<u8>,
    end: NonNull<u8>,
    capacity: usize,
}
