// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Empirical allocation gate for the 2CBH *framing* hot path only.
//! This does not claim a complete zero-allocation .2cbh-to-PGN converter.

use cbvault_format::twocbh::testdata::{Game, remove_set, write_set};
use cbvault_format::twocbh::{self, Headers, Segment};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

struct CountingAllocator;
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        #[allow(unsafe_code)]
        unsafe {
            System.alloc(layout)
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        #[allow(unsafe_code)]
        unsafe {
            System.alloc_zeroed(layout)
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        #[allow(unsafe_code)]
        unsafe {
            System.realloc(ptr, layout, size)
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        #[allow(unsafe_code)]
        unsafe {
            System.dealloc(ptr, layout)
        }
    }
}
fn count() {
    if !ARMED.try_with(Cell::get).unwrap_or(false) || INSIDE.try_with(Cell::get).unwrap_or(true) {
        return;
    }
    let _ = INSIDE.try_with(|x| x.set(true));
    let _ = COUNT.try_with(|x| x.set(x.get() + 1));
    let _ = INSIDE.try_with(|x| x.set(false));
}
fn measure(f: impl FnOnce()) -> u64 {
    let _ = COUNT.try_with(|x| x.set(0));
    let _ = ARMED.try_with(|x| x.set(true));
    f();
    let _ = ARMED.try_with(|x| x.set(false));
    COUNT.try_with(Cell::get).unwrap_or(u64::MAX)
}

#[test]
fn frame_read_into_reuses_buffer_without_allocating() {
    let stem = write_set("alloc-pread", &[Game::new(1, 1, 2)], true);
    let headers = Headers::open(&stem).unwrap();
    let segment = Segment::open(twocbh::sibling(&stem, ".2cbg")).unwrap();
    let offset = u64::try_from(headers.record(1).unwrap().moves_offset()).unwrap();
    let mut buffer = Vec::with_capacity(4096);
    segment.record_into(offset, &mut buffer).unwrap(); // warm up
    let allocations = measure(|| {
        for _ in 0..1024 {
            let frame = segment.record_into(offset, &mut buffer).unwrap();
            std::hint::black_box(frame.content());
        }
    });
    assert_eq!(allocations, 0, "caller-buffer frame reads allocated unexpectedly");
    remove_set(&stem);
}

#[test]
fn mmap_frame_borrows_without_allocating() {
    let stem = write_set("alloc-map", &[Game::new(2, 3, 4)], true);
    let headers = Headers::open(&stem).unwrap();
    let segment = Segment::open(twocbh::sibling(&stem, ".2cbg")).unwrap();
    let offset = u64::try_from(headers.record(1).unwrap().moves_offset()).unwrap();
    if segment.db_file().as_slice().is_some() {
        segment.record_at(offset).unwrap(); // warm up
        let allocations = measure(|| {
            for _ in 0..1024 {
                let frame = segment.record_at(offset).unwrap();
                std::hint::black_box(frame.bytes());
            }
        });
        assert_eq!(allocations, 0, "borrowed mapped frame reads allocated unexpectedly");
    }
    remove_set(&stem);
}
