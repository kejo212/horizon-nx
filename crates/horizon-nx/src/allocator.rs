//! The `top_down_alloc` feature's global allocator. Rust's blocks of 256 KiB and up go in the
//! top-down region at the top of the heap, the one NVK takes GPU memory from (bevy-horizon's
//! `patches/mesa-switch/`); smaller ones go to newlib's malloc, as with Rust's `System`.
//!
//! On a desktop, malloc puts large blocks in their own `mmap` mappings, away from its small ones,
//! so freeing one gives its memory back whole. devkitA64's newlib malloc has no `mmap`: Doug Lea's
//! malloc 2.6.5 would map blocks of 128 KiB and up (`DEFAULT_MMAP_THRESHOLD`), but newlib's
//! `libc/stdlib/_mallocr.c` sets `HAVE_MMAP` to 0. So a game's large blocks sit between its small
//! ones in the one heap. Once loading frees them, malloc's free space is holes, and a GPU block
//! that needs one contiguous run finds none: in Skate 3 run sk8g, a 64 MiB block failed with 503
//! MiB free in holes. Here the region plays mmap's part. A large block takes whole 64 KiB granules
//! there, and freeing it leaves a run that GPU blocks and later large blocks can use; at the
//! region's bottom, the run goes back to malloc.
//!
//! A large block the region can't place comes from malloc, and a small block malloc can't place
//! goes to the region if one of its runs is free. The `[Memory]` lines count both.
//!
//! Below the region, Rust's blocks come from a TLSF heap (tlsf_heap.rs) that takes its memory
//! from malloc in large pools: malloc's own `free` was half of the game's CPU time.
//!
//! Blocks of up to 8 KiB, aligned to at most 16 bytes, go through a cache in front of the heap
//! (small_blocks.rs) unless `BH_ALLOC_CACHE=0`; blocks between that and [`LARGE`] go through a
//! second cache (medium_blocks.rs) unless `BH_ALLOC_MEDIUM=0`.

mod medium_blocks;
mod small_blocks;
mod tlsf_heap;

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use medium_blocks::Cache as MediumCache;
use small_blocks::{Backend, Cache};

/// Blocks this big and up go to the region. A smaller one would waste more of its last granule.
pub(crate) const LARGE: usize = 256 << 10;
/// malloc's blocks from this size up to [`LARGE`] are counted, to show what a lower threshold
/// would move.
pub(crate) const MEDIUM: usize = 64 << 10;
/// The region's granule (`NOUVEAU_HORIZON_REGION_GRANULE_B`): its blocks are whole granules,
/// aligned to at least one.
const GRANULE: usize = 64 << 10;

unsafe extern "C" {
    /// A block in the region, or null when the region can't place it.
    fn nouveau_horizon_region_alloc(align: usize, size: usize) -> *mut u8;
    /// Frees a block, or the granules at the end of one, given the size asked for. Returns
    /// false, freeing nothing, when `ptr` isn't in the region.
    fn nouveau_horizon_region_free(ptr: *mut u8, size: usize) -> bool;
}

#[global_allocator]
static GLOBAL: HorizonAllocator = HorizonAllocator;

struct HorizonAllocator;

/// The caches' backend: the TLSF heap, every block aligned to the caches' [`MAX_ALIGN`].
struct Newlib;

impl Backend for Newlib {
    #[inline]
    unsafe fn alloc(size: usize, zeroed: bool) -> *mut u8 {
        match Layout::from_size_align(size, small_blocks::MAX_ALIGN) {
            Ok(layout) => unsafe { tlsf_heap::alloc(layout, zeroed) },
            Err(_) => core::ptr::null_mut(),
        }
    }

    #[inline]
    unsafe fn free(ptr: *mut u8) {
        unsafe { tlsf_heap::free(ptr, small_blocks::MAX_ALIGN) }
    }

    #[inline]
    unsafe fn usable_size(ptr: *mut u8) -> usize {
        unsafe { tlsf_heap::usable_size(ptr) }
    }

    /// The thread's local region (`tpidrro_el0`), which the kernel gives each thread, 0x200 bytes
    /// apart: always set, unlike `tpidr_el0` before compat.rs sets it on a new thread.
    #[inline]
    fn shard_hint() -> usize {
        let tlr: usize;
        unsafe {
            core::arch::asm!("mrs {}, tpidrro_el0", out(reg) tlr, options(nomem, nostack, preserves_flags))
        };
        tlr >> 9
    }
}

static CACHE: Cache<Newlib> = Cache::new();
/// The medium-block cache's malloc, for blocks between the small cache and the region.
static MCACHE: MediumCache<Newlib> = MediumCache::new();

/// Reads `BH_ALLOC_CACHE` and `BH_ALLOC_MEDIUM`: `0` turns the small- and medium-block caches off,
/// giving back the blocks they hold. `HorizonPlugin` calls this once, after the game has set its
/// environment; until then both caches are on.
pub(crate) fn configure_from_env() {
    if std::env::var_os("BH_ALLOC_CACHE").is_some_and(|value| value == "0") {
        CACHE.set_enabled(false);
        log::info!("BH_ALLOC_CACHE=0: the small-block cache is off");
    }
    if std::env::var_os("BH_ALLOC_MEDIUM").is_some_and(|value| value == "0") {
        MCACHE.set_enabled(false);
        log::info!("BH_ALLOC_MEDIUM=0: the medium-block cache is off");
    }
    if let Some(mib) = std::env::var("BH_ALLOC_DEPOT_MIB").ok().and_then(|v| v.trim().parse::<usize>().ok()) {
        CACHE.set_depot_limit(mib << 20);
        crate::logging::write_log_line(format_args!(
            "[Boot] BH_ALLOC_DEPOT_MIB={mib}: the small-block cache's depot keeps up to {mib} MiB\n"
        ));
    }
}

/// Bytes, in whole granules, in Rust's blocks in the region.
static REGION_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Bytes in Rust's malloc blocks of [`LARGE`] and up.
static MALLOC_LARGE_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Bytes in Rust's malloc blocks from [`MEDIUM`] up to [`LARGE`].
static MALLOC_MEDIUM_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Large blocks the region couldn't place, which malloc took.
static LARGE_FALLBACKS: AtomicUsize = AtomicUsize::new(0);
/// Small blocks malloc couldn't place, which the region took.
static SMALL_RESCUES: AtomicUsize = AtomicUsize::new(0);
/// Small blocks for which both malloc and the region came up empty.
static SMALL_RESCUE_FAILURES: AtomicUsize = AtomicUsize::new(0);
/// Failures written to `log.txt` so far. The first few are reported, so the error path's own
/// allocations can't fill the log, but a burst from several threads still shows.
static FAILURES: AtomicUsize = AtomicUsize::new(0);
/// The most allocation failures to report.
const MAX_FAILURE_REPORTS: usize = 8;

/// Where Rust's memory is, for the `[Memory]` lines. Reading it doesn't allocate.
#[derive(Clone, Copy)]
pub(crate) struct Figures {
    pub(crate) region: u64,
    pub(crate) malloc_large: u64,
    pub(crate) malloc_medium: u64,
    pub(crate) large_fallbacks: usize,
    pub(crate) small_rescues: usize,
    /// Bytes the TLSF heap took from malloc.
    pub(crate) heap_pools: u64,
    /// Bytes in the TLSF heap's live blocks, cached ones included.
    pub(crate) heap_live: u64,
    /// Times a TLSF pool could not be taken from malloc, and the last pool size tried.
    pub(crate) tlsf_grow_failures: usize,
    pub(crate) tlsf_last_grow: u64,
    /// Small blocks for which both malloc and the region came up empty.
    pub(crate) small_rescue_failures: usize,
    pub(crate) cache: small_blocks::Stats,
    pub(crate) medium: medium_blocks::Stats,
}

pub(crate) fn figures() -> Figures {
    Figures {
        region: REGION_BYTES.load(Relaxed) as u64,
        malloc_large: MALLOC_LARGE_BYTES.load(Relaxed) as u64,
        malloc_medium: MALLOC_MEDIUM_BYTES.load(Relaxed) as u64,
        large_fallbacks: LARGE_FALLBACKS.load(Relaxed),
        small_rescues: SMALL_RESCUES.load(Relaxed),
        heap_pools: tlsf_heap::POOL_BYTES.load(Relaxed) as u64,
        heap_live: tlsf_heap::LIVE_BYTES.load(Relaxed) as u64,
        tlsf_grow_failures: tlsf_heap::GROW_FAILURES.load(Relaxed),
        tlsf_last_grow: tlsf_heap::LAST_GROW_BYTES.load(Relaxed) as u64,
        small_rescue_failures: SMALL_RESCUE_FAILURES.load(Relaxed),
        cache: CACHE.stats(),
        medium: MCACHE.stats(),
    }
}

/// The granules a block of `size` bytes takes in the region. Can't overflow: a `Layout`'s size is
/// at most `isize::MAX`.
fn rounded(size: usize) -> usize {
    size.div_ceil(GRANULE) * GRANULE
}

/// Whether a block is the region's. malloc's blocks lie below `sbrk`'s break, which never passes
/// `fake_heap_end`, and the region's lie at or above it. NVK moves `fake_heap_end` only past free
/// memory (it grows the region only 16 MiB or more above the break, and shrinks it only up to
/// its lowest block), so the answer for a live block never changes and needs no lock.
fn in_region(ptr: *mut u8) -> bool {
    ptr as usize >= crate::memory::heap_limit()
}

fn count_malloc(size: usize, added: bool) {
    let counter = if size >= LARGE {
        &MALLOC_LARGE_BYTES
    } else if size >= MEDIUM {
        &MALLOC_MEDIUM_BYTES
    } else {
        return;
    };
    if added {
        counter.fetch_add(size, Relaxed);
    } else {
        counter.fetch_sub(size, Relaxed);
    }
}

unsafe fn region_alloc(layout: Layout, zeroed: bool) -> *mut u8 {
    let ptr = unsafe { nouveau_horizon_region_alloc(layout.align(), layout.size()) };
    if !ptr.is_null() {
        REGION_BYTES.fetch_add(rounded(layout.size()), Relaxed);
        // A granule can come back from an earlier block, so it isn't zero.
        if zeroed {
            unsafe { ptr.write_bytes(0, layout.size()) };
        }
    }
    ptr
}

unsafe fn malloc_alloc(layout: Layout, zeroed: bool) -> *mut u8 {
    let ptr = unsafe { tlsf_heap::alloc(layout, zeroed) };
    if !ptr.is_null() {
        count_malloc(layout.size(), true);
    }
    ptr
}

/// `ptr`, or if it's null because malloc ran out, what `again` gives once both caches have given
/// malloc their blocks.
#[inline]
fn or_after_drain(ptr: *mut u8, again: impl FnOnce() -> *mut u8) -> *mut u8 {
    if ptr.is_null() {
        let gave = CACHE.drain();
        if MCACHE.drain() || gave {
            return again();
        }
    }
    ptr
}

unsafe fn allocate(layout: Layout, zeroed: bool) -> *mut u8 {
    if layout.size() >= LARGE {
        let ptr = unsafe { region_alloc(layout, zeroed) };
        if !ptr.is_null() {
            return ptr;
        }
        LARGE_FALLBACKS.fetch_add(1, Relaxed);
        let ptr = unsafe { malloc_alloc(layout, zeroed) };
        return or_after_drain(ptr, || unsafe { malloc_alloc(layout, zeroed) });
    }
    let small = || match CACHE.class_for(layout.size(), layout.align()) {
        Some(class) => unsafe { CACHE.alloc(class, layout.size(), zeroed) },
        None => match MCACHE.class_for(layout.size(), layout.align()) {
            Some(class) => {
                let ptr = unsafe { MCACHE.alloc(class, layout.size(), zeroed) };
                if !ptr.is_null() {
                    count_malloc(layout.size(), true);
                }
                ptr
            }
            None => unsafe { malloc_alloc(layout, zeroed) },
        },
    };
    let ptr = or_after_drain(small(), small);
    if !ptr.is_null() {
        return ptr;
    }
    // malloc is full, but a free run in the region can still hold the block.
    let ptr = unsafe { region_alloc(layout, zeroed) };
    if !ptr.is_null() {
        SMALL_RESCUES.fetch_add(1, Relaxed);
    } else {
        SMALL_RESCUE_FAILURES.fetch_add(1, Relaxed);
    }
    ptr
}

unsafe fn deallocate(ptr: *mut u8, layout: Layout) {
    if in_region(ptr) {
        // False can't happen, by `in_region`'s reasoning; leaking the block is safer than giving
        // malloc memory that isn't its own.
        if unsafe { nouveau_horizon_region_free(ptr, layout.size()) } {
            REGION_BYTES.fetch_sub(rounded(layout.size()), Relaxed);
        }
        return;
    }
    if let Some(class) = CACHE.class_for(layout.size(), layout.align()) {
        return unsafe { CACHE.free(ptr, class) };
    }
    if let Some(class) = MCACHE.class_for(layout.size(), layout.align()) {
        count_malloc(layout.size(), false);
        return unsafe { MCACHE.free(ptr, class) };
    }
    count_malloc(layout.size(), false);
    unsafe { tlsf_heap::free(ptr, layout.align()) }
}

/// Shrinks a region block in place, freeing its granules past `new_rounded`.
unsafe fn release_tail(ptr: *mut u8, new_rounded: usize, old_rounded: usize) {
    let tail = old_rounded - new_rounded;
    if unsafe { nouveau_horizon_region_free(ptr.add(new_rounded), tail) } {
        REGION_BYTES.fetch_sub(tail, Relaxed);
    }
}

/// Copies a block to `new` and frees it.
unsafe fn moved(ptr: *mut u8, layout: Layout, new: *mut u8, new_size: usize) -> *mut u8 {
    unsafe {
        core::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
        deallocate(ptr, layout);
    }
    new
}

/// Writes a failed allocation to `log.txt` before Rust's allocation-error handler aborts the
/// process: the block, the thread's TLS pointer, the caller's return addresses, and the heap's
/// figures. Allocates nothing, so it can run when the allocator has just failed.
#[cfg(target_os = "horizon")]
#[cold]
#[inline(never)]
fn report_failure(layout: Layout) {
    use crate::logging::{write_log_line, write_log_line_long};

    let n = FAILURES.fetch_add(1, Relaxed);
    if n >= MAX_FAILURE_REPORTS {
        return;
    }
    let (sp, fp, tls): (usize, usize, usize);
    unsafe {
        core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov {}, x29", out(reg) fp, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mrs {}, tpidrro_el0", out(reg) tls, options(nomem, nostack, preserves_flags));
    }
    write_log_line(format_args!(
        "[Alloc] FAILED #{}: {} bytes, align {}; thread tls {tls:#x}, sp {sp:#x}, fp {fp:#x}",
        n + 1,
        layout.size(),
        layout.align()
    ));
    // The stack comes before the figures: if the heap's bookkeeping is what broke, `mallinfo`
    // may fault, and the call chain is the part worth keeping.
    let mut frames = [0usize; 32];
    let count = unsafe { stack_frames(&mut frames) };
    let base = crate::logging::nro_base() as usize;
    for &frame in &frames[..count] {
        write_log_line(format_args!(
            "[Alloc]   at {frame:#x} (nro+{:#x})",
            frame.wrapping_sub(base)
        ));
    }
    write_log_line_long(format_args!(
        "[Alloc] {}",
        crate::memory::MallocFigures::now()
    ));
}

/// The calling thread's return addresses, innermost first, from its frame-pointer chain. The
/// target keeps frame pointers (`"frame-pointer": "always"` in bevy-horizon's target spec).
/// Reads only the current thread's stack, found with `svcQueryMemory`, so a bad link stops the
/// walk instead of faulting.
#[cfg(target_os = "horizon")]
#[inline(never)]
unsafe fn stack_frames(out: &mut [usize]) -> usize {
    use crate::logging::ffi;

    let (sp, fp): (usize, usize);
    unsafe {
        core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mov {}, x29", out(reg) fp, options(nomem, nostack, preserves_flags));
    }
    let mut info = ffi::MemoryInfo::default();
    let mut page = 0u32;
    if unsafe { ffi::svcQueryMemory(&mut info, &mut page, sp as u64) } != 0 {
        return 0;
    }
    let lo = info.addr as usize;
    let hi = info.addr.saturating_add(info.size) as usize;
    let mut n = 0;
    let mut frame = fp;
    let mut floor = sp;
    while n < out.len()
        && frame % 16 == 0
        && frame >= floor
        && frame >= lo
        && frame <= hi.saturating_sub(16)
    {
        let (next, ret) = unsafe { (*(frame as *const usize), *((frame + 8) as *const usize)) };
        if ret == 0 {
            break;
        }
        out[n] = ret;
        n += 1;
        floor = frame + 1;
        frame = next;
    }
    n
}

unsafe impl GlobalAlloc for HorizonAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { allocate(layout, false) };
        if ptr.is_null() {
            report_failure(layout);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { allocate(layout, true) };
        if ptr.is_null() {
            report_failure(layout);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { deallocate(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { reallocate(ptr, layout, new_size) };
        if new.is_null() {
            // SAFETY: the caller guarantees `new_size`, rounded up to the alignment, doesn't
            // overflow `isize`.
            report_failure(unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) });
        }
        new
    }
}

/// The body of [`GlobalAlloc::realloc`], so the trait method can report a failure.
unsafe fn reallocate(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    // SAFETY: `GlobalAlloc::realloc`'s caller guarantees `new_size`, rounded up to the
    // alignment, doesn't overflow `isize`.
    let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };

    if in_region(ptr) {
        let (old_rounded, new_rounded) = (rounded(layout.size()), rounded(new_size));
        if new_rounded == old_rounded {
            return ptr;
        }
        if new_rounded < old_rounded && new_size >= LARGE {
            unsafe { release_tail(ptr, new_rounded, old_rounded) };
            return ptr;
        }
        // Growing, or small enough for malloc now.
        let new = unsafe { allocate(new_layout, false) };
        if !new.is_null() {
            return unsafe { moved(ptr, layout, new, new_size) };
        }
        if new_rounded < old_rounded {
            unsafe { release_tail(ptr, new_rounded, old_rounded) };
            return ptr;
        }
        return new;
    }

    let (old_small, new_small) = (
        CACHE.class_for(layout.size(), layout.align()),
        CACHE.class_for(new_size, layout.align()),
    );
    let (old_medium, new_medium) = (
        MCACHE.class_for(layout.size(), layout.align()),
        MCACHE.class_for(new_size, layout.align()),
    );
    if old_small.is_some() || new_small.is_some() || old_medium.is_some() || new_medium.is_some() {
        // A block that stays in its class stays put: it was allocated at the class's size,
        // unless the cache was off then.
        if old_small.is_some() && old_small == new_small && unsafe { CACHE.usable_size(ptr) } >= new_size {
            return ptr;
        }
        if old_medium.is_some() && old_medium == new_medium && unsafe { MCACHE.usable_size(ptr) } >= new_size {
            // The block stays live and parked-or-held; only its counted size changes, and only
            // in the 64-256 KiB band `count_malloc` tracks. `count_malloc` ignores a size under
            // MEDIUM, so a block that straddles it (both in the 64 KiB class) is handled.
            if layout.size() >= MEDIUM || new_size >= MEDIUM {
                count_malloc(layout.size(), false);
                count_malloc(new_size, true);
            }
            return ptr;
        }
        // In or out of a cache's sizes, or to another class: a new block, so that every block
        // a cache keeps is its class's size.
        let new = unsafe { allocate(new_layout, false) };
        if new.is_null() {
            return new;
        }
        return unsafe { moved(ptr, layout, new, new_size) };
    }

    if new_size >= LARGE {
        let new = unsafe { region_alloc(new_layout, false) };
        if !new.is_null() {
            return unsafe { moved(ptr, layout, new, new_size) };
        }
        LARGE_FALLBACKS.fetch_add(1, Relaxed);
    }
    let new = unsafe { tlsf_heap::realloc(ptr, layout, new_size) };
    // A failed realloc leaves the block as it was.
    let new = or_after_drain(new, || unsafe { tlsf_heap::realloc(ptr, layout, new_size) });
    if !new.is_null() {
        count_malloc(layout.size(), false);
        count_malloc(new_size, true);
        return new;
    }
    if new_size >= LARGE {
        return new;
    }
    // malloc is full, but a free run in the region can still hold the block.
    let new = unsafe { region_alloc(new_layout, false) };
    if new.is_null() {
        SMALL_RESCUE_FAILURES.fetch_add(1, Relaxed);
        return new;
    }
    SMALL_RESCUES.fetch_add(1, Relaxed);
    unsafe { moved(ptr, layout, new, new_size) }
}
