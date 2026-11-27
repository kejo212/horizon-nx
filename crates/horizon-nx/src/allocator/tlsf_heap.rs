//! Rust's blocks below the region's ([`super::LARGE`]), in a TLSF heap instead of newlib's malloc.
//!
//! devkitA64's newlib malloc is Doug Lea's malloc 2.6.5. Its `free` sorts every block of 512 bytes
//! and up into its bin by size, walking the bin, so freeing a big tree of such blocks takes time
//! that grows with how many free blocks the bin already holds. On a console (2026-11-19, the
//! measuring build's cpu-profile.txt) that was half of all the game's CPU time: the thread loading
//! block behaviours spent 93% of its 74 s in `_free_r` dropping one parsed JSON tree, the mod
//! content thread 90% in `_malloc_r` and `_free_r`, the main thread about 40% in allocation.
//!
//! TLSF (two-level segregated fit, the `rlsf` crate) allocates and frees in constant time. Its
//! memory comes from newlib's malloc in pools of [`POOL`] or more, which it keeps: the game's small
//! blocks come and go, and a kept pool serves the next ones. The heap is behind a libnx mutex,
//! which the kernel arbitrates by priority (a spinning thread could starve a lower-priority holder
//! on its own core); the small- and medium-block caches in front take most calls without it.

use core::alloc::Layout;
use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering::Relaxed};

/// Blocks of up to 2 GiB (32 bytes << 26), 16 sizes per power of two.
type Heap = rlsf::Tlsf<'static, u32, u16, 26, 16>;

/// The least memory taken from malloc at a time.
const POOL: usize = 32 << 20;
/// Pools to try when [`POOL`] can't be taken from malloc. The top-down region leaves only
/// `NOUVEAU_HORIZON_REGION_CPU_RESERVE_B` (16 MiB in the linked NVK) above the CPU heap's break,
/// so once the region has grown a 32 MiB `memalign` can fail even with a GiB free in the heap's
/// holes; a smaller pool still serves the block that needed it.
const POOL_FALLBACKS: [usize; 2] = [8 << 20, 1 << 20];

/// The heap, and the libnx mutex guarding it (a `u32`, 0 when unlocked).
static mut HEAP: Heap = Heap::new();
static LOCK: AtomicU32 = AtomicU32::new(0);

/// Bytes taken from malloc for pools.
pub(crate) static POOL_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Times a pool could not be taken from malloc, and the size of the last such pool.
pub(crate) static GROW_FAILURES: AtomicUsize = AtomicUsize::new(0);
pub(crate) static LAST_GROW_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Times a smaller fallback pool served the block because the full pool didn't fit.
pub(crate) static FALLBACK_POOLS: AtomicUsize = AtomicUsize::new(0);
/// Bytes in the heap's live blocks (their usable sizes), the small and medium caches' blocks
/// included: the pools never shrink, so this says how much of them a session still uses.
pub(crate) static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" {
    fn memalign(align: usize, size: usize) -> *mut c_void;
}

struct Locked;

impl Locked {
    fn new() -> Self {
        unsafe { crate::logging::ffi::mutexLock(LOCK.as_ptr()) };
        Locked
    }
    /// The heap, while this guard holds the lock.
    // A plain `&mut HEAP` is refused for a `static mut` (edition 2024); the raw pointer isn't.
    #[allow(clippy::deref_addrof)]
    fn heap(&mut self) -> &mut Heap {
        // SAFETY: only reached with LOCK held, so no other reference to HEAP is live.
        unsafe { &mut *(&raw mut HEAP) }
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        unsafe { crate::logging::ffi::mutexUnlock(LOCK.as_ptr()) };
    }
}

/// Gives the heap a pool that can hold a block of `layout`; false when malloc has no room.
fn grow(heap: &mut Heap, layout: Layout) -> bool {
    // Room for the block, its alignment, and the heap's headers.
    let Some(needed) = layout
        .size()
        .checked_add(layout.align())
        .and_then(|n| n.checked_add(4096))
    else {
        return false;
    };
    let wanted = needed.max(POOL).next_multiple_of(4096);
    for size in [wanted, POOL_FALLBACKS[0], POOL_FALLBACKS[1]] {
        if size < needed {
            continue;
        }
        let pool = unsafe { memalign(4096, size) }.cast::<u8>();
        let Some(pool) = NonNull::new(pool) else {
            continue;
        };
        POOL_BYTES.fetch_add(size, Relaxed);
        if size < wanted {
            FALLBACK_POOLS.fetch_add(1, Relaxed);
        }
        // SAFETY: the pool is ours from now on and never given back, so it outlives the heap.
        unsafe { heap.insert_free_block_ptr(NonNull::slice_from_raw_parts(pool, size)) };
        return true;
    }
    GROW_FAILURES.fetch_add(1, Relaxed);
    LAST_GROW_BYTES.store(wanted, Relaxed);
    false
}

/// A block of `layout`, zeroed if `zeroed`, or null when neither the heap nor malloc has room.
pub(crate) unsafe fn alloc(layout: Layout, zeroed: bool) -> *mut u8 {
    let ptr = {
        let mut held = Locked::new();
        let heap = held.heap();
        let ptr = match heap.allocate(layout) {
            Some(ptr) => ptr,
            None if grow(heap, layout) => match heap.allocate(layout) {
                Some(ptr) => ptr,
                None => return core::ptr::null_mut(),
            },
            None => return core::ptr::null_mut(),
        };
        // SAFETY: a block the heap just gave.
        LIVE_BYTES.fetch_add(unsafe { Heap::allocation_usable_size(ptr) }, Relaxed);
        ptr
    };
    if zeroed {
        unsafe { ptr.as_ptr().write_bytes(0, layout.size()) };
    }
    ptr.as_ptr()
}

/// Frees a block [`alloc`] gave for `align`.
pub(crate) unsafe fn free(ptr: *mut u8, align: usize) {
    let Some(ptr) = NonNull::new(ptr) else { return };
    let mut held = Locked::new();
    LIVE_BYTES.fetch_sub(unsafe { Heap::allocation_usable_size(ptr) }, Relaxed);
    unsafe { held.heap().deallocate(ptr, align) };
}

/// The bytes a live block can hold (at least what it was allocated for).
pub(crate) unsafe fn usable_size(ptr: *mut u8) -> usize {
    match NonNull::new(ptr) {
        Some(ptr) => unsafe { Heap::allocation_usable_size(ptr) },
        None => 0,
    }
}

/// `ptr`'s block as `new_size` bytes (same alignment), in place when it can be, or null, leaving
/// the block as it was.
pub(crate) unsafe fn realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    let Some(old) = NonNull::new(ptr) else {
        return core::ptr::null_mut();
    };
    let Ok(new_layout) = Layout::from_size_align(new_size, layout.align()) else {
        return core::ptr::null_mut();
    };
    {
        let mut held = Locked::new();
        let before = unsafe { Heap::allocation_usable_size(old) };
        if let Some(moved) = unsafe { held.heap().reallocate(old, new_layout) } {
            let after = unsafe { Heap::allocation_usable_size(moved) };
            LIVE_BYTES.fetch_add(after, Relaxed);
            LIVE_BYTES.fetch_sub(before, Relaxed);
            return moved.as_ptr();
        }
    }
    // No room in the heap as it is: a new block (which grows it), then the bytes, then the old one.
    let new = unsafe { alloc(new_layout, false) };
    if !new.is_null() {
        unsafe {
            core::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
            free(ptr, layout.align());
        }
    }
    new
}
