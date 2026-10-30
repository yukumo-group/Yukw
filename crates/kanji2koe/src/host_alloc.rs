//! C allocator shims for the statically linked engines.
//!
//! `wasm32-unknown-unknown` Rust does not export the C `malloc` family, so the
//! relocatable AquesTalk archives would have undefined symbols.  These wrappers
//! forward to Rust's global allocator with a 16-byte header so `free` and
//! `realloc` can recover the layout.  Allocations are zeroed: AqKanji2Koe and
//! AqUsrDic were built with an allocator that zero-fills reused small blocks,
//! and the decompiled code relies on that.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ptr;

const HEADER: usize = 16;
const ALIGN: usize = 16;

fn layout_for(total: usize) -> Option<Layout> {
    let size = total.checked_add(ALIGN - 1)? & !(ALIGN - 1);
    Layout::from_size_align(size, ALIGN).ok()
}

fn allocate(size: usize) -> *mut u8 {
    let Some(total) = size.checked_add(HEADER) else {
        return ptr::null_mut();
    };
    let Some(layout) = layout_for(total) else {
        return ptr::null_mut();
    };
    let base = unsafe { alloc_zeroed(layout) };
    if base.is_null() {
        return ptr::null_mut();
    }
    // `layout` covers the header and the payload.
    unsafe {
        base.cast::<usize>().write(total);
        base.add(HEADER)
    }
}

/// # Safety
/// `payload` came from [`allocate`] and has not been freed.
unsafe fn allocated_block(payload: *mut u8) -> (*mut u8, usize) {
    unsafe {
        let base = payload.sub(HEADER);
        (base, base.cast::<usize>().read())
    }
}

#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    allocate(size.max(1))
}

#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
    let Some(total) = count.checked_mul(size) else {
        return ptr::null_mut();
    };
    allocate(total.max(1))
}

/// # Safety
/// `pointer` is null, or a live pointer from [`malloc`], [`calloc`], or [`realloc`].
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub unsafe extern "C" fn free(pointer: *mut u8) {
    if pointer.is_null() {
        return;
    }
    let (base, total) = unsafe { allocated_block(pointer) };
    let layout = layout_for(total).expect("stored size has a valid layout");
    unsafe { dealloc(base, layout) };
}

/// # Safety
/// `pointer` is null, or a live pointer from [`malloc`], [`calloc`], or [`realloc`].
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub unsafe extern "C" fn realloc(pointer: *mut u8, size: usize) -> *mut u8 {
    if pointer.is_null() {
        return malloc(size);
    }
    if size == 0 {
        unsafe { free(pointer) };
        return ptr::null_mut();
    }
    let old_size = unsafe { allocated_block(pointer) }.1 - HEADER;
    let new = allocate(size);
    if new.is_null() {
        return ptr::null_mut();
    }
    // Both blocks are live; copy only the shared prefix.
    unsafe { ptr::copy_nonoverlapping(pointer, new, old_size.min(size)) };
    unsafe { free(pointer) };
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_aligned(pointer: *mut u8) {
        assert!(!pointer.is_null());
        assert_eq!(pointer as usize % ALIGN, 0);
    }

    fn paint(pointer: *mut u8, size: usize, seed: u8) {
        for (index, byte) in unsafe { std::slice::from_raw_parts_mut(pointer, size) }
            .iter_mut()
            .enumerate()
        {
            *byte = seed.wrapping_add(index as u8);
        }
    }

    fn assert_paint(pointer: *mut u8, size: usize, seed: u8) {
        for (index, byte) in unsafe { std::slice::from_raw_parts(pointer, size) }
            .iter()
            .enumerate()
        {
            assert_eq!(
                *byte,
                seed.wrapping_add(index as u8),
                "byte {index} of a {size}-byte block"
            );
        }
    }

    fn assert_zero(pointer: *mut u8, size: usize) {
        assert!(
            unsafe { std::slice::from_raw_parts(pointer, size) }
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn zero_size_requests_are_distinct_and_zeroed() {
        let pointers = [
            malloc(0),
            calloc(0, 4),
            calloc(4, 0),
            calloc(0, 0),
            unsafe { realloc(ptr::null_mut(), 0) },
        ];
        for (index, pointer) in pointers.iter().enumerate() {
            assert_aligned(*pointer);
            assert_zero(*pointer, 1);
            for other in &pointers[index + 1..] {
                assert_ne!(*pointer, *other);
            }
        }
        unsafe { free(ptr::null_mut()) };
        for pointer in pointers {
            unsafe { free(pointer) };
        }
    }

    #[test]
    fn overflow_returns_null() {
        assert!(malloc(usize::MAX).is_null());
        assert!(malloc(usize::MAX - (HEADER - 1)).is_null());
        assert!(malloc(isize::MAX as usize).is_null());
        assert!(calloc(usize::MAX, usize::MAX).is_null());
        assert!(calloc(usize::MAX / 2 + 1, 2).is_null());
        let half = 1usize << (usize::BITS / 2);
        assert!(calloc(half, half).is_null());
    }

    #[test]
    fn failed_realloc_keeps_the_old_block() {
        let pointer = malloc(32);
        assert_aligned(pointer);
        paint(pointer, 32, 9);
        assert!(unsafe { realloc(pointer, usize::MAX) }.is_null());
        assert_paint(pointer, 32, 9);
        assert!(unsafe { realloc(pointer, isize::MAX as usize) }.is_null());
        assert_paint(pointer, 32, 9);
        unsafe { free(pointer) };
    }

    #[test]
    fn realloc_grows_shrinks_and_zero_fills() {
        let mut pointer = malloc(1);
        assert_aligned(pointer);
        paint(pointer, 1, 3);

        for size in [1, 15, 16, 17, 31, 32, 33, 63, 64, 127, 128, 129, 4096, 4097] {
            let next = unsafe { realloc(pointer, size) };
            assert_aligned(next);
            assert_paint(next, 1, 3);
            assert_zero(unsafe { next.add(1) }, size - 1);
            paint(next, size, 3);
            pointer = next;

            let shrunk = unsafe { realloc(pointer, 1) };
            assert_aligned(shrunk);
            assert_paint(shrunk, 1, 3);
            pointer = shrunk;
        }

        assert!(unsafe { realloc(pointer, 0) }.is_null());
    }

    #[test]
    fn realloc_preserves_every_prefix_byte() {
        let size = 300usize;
        let pointer = calloc(10, 30);
        assert_aligned(pointer);
        assert_zero(pointer, size);
        paint(pointer, size, 21);

        let grown = unsafe { realloc(pointer, size + 40) };
        assert_aligned(grown);
        assert_ne!(
            grown, pointer,
            "copy must happen while the old block is live"
        );
        assert_paint(grown, size, 21);
        assert_zero(unsafe { grown.add(size) }, 40);

        let shrunk = unsafe { realloc(grown, 17) };
        assert_aligned(shrunk);
        assert_paint(shrunk, 17, 21);
        unsafe { free(shrunk) };
    }

    #[test]
    fn stored_header_is_the_requested_total() {
        let size = 24usize;
        let pointer = malloc(size);
        let total = unsafe { allocated_block(pointer) }.1;
        assert_eq!(total, size + HEADER);
        unsafe { free(pointer) };
    }

    #[test]
    fn reuse_after_free_is_zeroed() {
        let pointer = malloc(128);
        paint(pointer, 128, 0xA5);
        unsafe { free(pointer) };
        let again = calloc(128, 1);
        assert_aligned(again);
        assert_zero(again, 128);
        unsafe { free(again) };
    }

    #[test]
    fn neighboring_blocks_do_not_overlap() {
        let sizes = [
            1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 255, 256, 1024, 4095, 4096, 4097, 8192,
        ];
        let mut live: Vec<(*mut u8, usize, u8)> = sizes
            .into_iter()
            .enumerate()
            .map(|(index, size)| {
                let pointer = if index % 2 == 0 {
                    malloc(size)
                } else {
                    calloc(size, 1)
                };
                assert_aligned(pointer);
                assert_zero(pointer, size);
                let seed = index as u8 + 1;
                paint(pointer, size, seed);
                (pointer, size, seed)
            })
            .collect();

        for (pointer, size, seed) in &live {
            assert_paint(*pointer, *size, *seed);
        }

        let mut replacements = Vec::new();
        for (index, block) in live.iter_mut().enumerate() {
            if index % 2 == 0 {
                continue;
            }
            let (pointer, size, seed) = *block;
            let next = unsafe { realloc(pointer, size.saturating_mul(2).max(1)) };
            assert_aligned(next);
            assert_paint(next, size, seed);
            assert_zero(unsafe { next.add(size) }, size);
            paint(next, size * 2, seed);
            replacements.push((index, next, size * 2, seed));
        }
        for (index, pointer, size, seed) in replacements {
            live[index] = (pointer, size, seed);
        }

        for (pointer, size, seed) in &live {
            assert_paint(*pointer, *size, *seed);
        }
        for (pointer, _, _) in live {
            unsafe { free(pointer) };
        }
    }

    #[test]
    fn stress_mixed_operations() {
        struct Live {
            pointer: *mut u8,
            size: usize,
            seed: u8,
        }

        let mut state = 0x1234_5678u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state
        };
        let mut live = Vec::<Live>::new();

        for step in 0..4_000 {
            match next() % 5 {
                0 | 1 => {
                    if live.len() >= 128 {
                        continue;
                    }
                    let size = (next() as usize % 2048) + 1;
                    let seed = (next() % 251) as u8 + 1;
                    let pointer = if next() % 2 == 0 {
                        malloc(size)
                    } else {
                        calloc(size, 1)
                    };
                    assert_aligned(pointer);
                    assert_zero(pointer, size);
                    paint(pointer, size, seed);
                    live.push(Live {
                        pointer,
                        size,
                        seed,
                    });
                }
                2 => {
                    if live.is_empty() {
                        continue;
                    }
                    let index = next() as usize % live.len();
                    let block = &live[index];
                    let new_size = (next() as usize % 2048) + 1;
                    let pointer = unsafe { realloc(block.pointer, new_size) };
                    assert_aligned(pointer);
                    let kept = block.size.min(new_size);
                    assert_paint(pointer, kept, block.seed);
                    if new_size > block.size {
                        assert_zero(unsafe { pointer.add(block.size) }, new_size - block.size);
                    }
                    paint(pointer, new_size, block.seed);
                    live[index] = Live {
                        pointer,
                        size: new_size,
                        seed: block.seed,
                    };
                }
                _ => {
                    if live.is_empty() {
                        continue;
                    }
                    let index = next() as usize % live.len();
                    let block = live.swap_remove(index);
                    unsafe { free(block.pointer) };
                }
            }

            if step % 250 == 0 {
                for block in &live {
                    assert_paint(block.pointer, block.size, block.seed);
                }
            }
        }

        for block in live {
            assert_paint(block.pointer, block.size, block.seed);
            unsafe { free(block.pointer) };
        }
    }
}
