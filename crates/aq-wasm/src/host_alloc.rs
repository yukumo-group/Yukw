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

