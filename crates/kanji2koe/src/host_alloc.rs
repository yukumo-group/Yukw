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

#[inline]
unsafe fn layout_for(total: usize) -> Layout {
    let size = (total + (ALIGN - 1)) & !(ALIGN - 1);
    unsafe { Layout::from_size_align_unchecked(size, ALIGN) }
}

unsafe fn allocate(size: usize) -> *mut u8 {
    let Some(total) = size.checked_add(HEADER) else {
        return ptr::null_mut();
    };
    let base = unsafe { alloc_zeroed(layout_for(total)) };
    if base.is_null() {
        return base;
    }
    unsafe { (base as *mut usize).write(total) };
    unsafe { base.add(HEADER) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    unsafe { allocate(size.max(1)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
    let Some(total) = count.checked_mul(size) else {
        return ptr::null_mut();
    };
    unsafe { allocate(total.max(1)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(pointer: *mut u8) {
    if pointer.is_null() {
        return;
    }
    let base = unsafe { pointer.sub(HEADER) };
    let total = unsafe { (base as *mut usize).read() };
    unsafe { dealloc(base, layout_for(total)) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(pointer: *mut u8, size: usize) -> *mut u8 {
    if pointer.is_null() {
        return unsafe { malloc(size) };
    }
    if size == 0 {
        unsafe { free(pointer) };
        return ptr::null_mut();
    }
    let base = unsafe { pointer.sub(HEADER) };
    let total = unsafe { (base as *mut usize).read() };
    let old_size = total - HEADER;
    let new = unsafe { allocate(size) };
    if new.is_null() {
        return new;
    }
    unsafe { ptr::copy_nonoverlapping(pointer, new, old_size.min(size)) };
    unsafe { free(pointer) };
    new
}
