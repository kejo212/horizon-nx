//! The SD card device's side of the resources pack (`pack.rs`): which folder is packed, the open
//! pack, the files opened from it, and the background build.

use super::pack::{self, Entry, Found, Index, Kind};
use super::{Parts, normalize, render};
use core::ffi::{c_int, c_void};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

type Stat = libc::stat;

unsafe extern "C" {
    /// libnx: reads `size` bytes at `off` of an open file, by offset (no shared position).
    fn fsFileRead(
        f: *mut c_void,
        off: i64,
        buf: *mut c_void,
        size: u64,
        option: u32,
        read: *mut u64,
    ) -> u32;
}

/// A packed folder in use.
pub(super) struct Mount {
    /// The folder, as absolute parts.
    root: Parts,
    folder: PathBuf,
    pub(super) index: Index,
    /// libnx's file record for the open pack (`fsdev_file_t`, whose first field is the `FsFile`).
    file: Vec<u64>,
    /// What the folder was built from, for a cache folder ([`use_cache_pack`]).
    stamp: Option<String>,
}

impl Drop for Mount {
    fn drop(&mut self) {
        let o = super::devoptab::original();
        unsafe { o.close_r.unwrap()(super::devoptab::reent(), self.file.as_mut_ptr().cast()) };
    }
}

/// The packs in use: the resources folder's, and cache folders' ([`use_cache_pack`]). A file
/// open from one keeps it alive when it is dropped from here.
static MOUNTS: RwLock<Vec<Arc<Mount>>> = RwLock::new(Vec::new());

/// A folder that has or will have a pack.
struct Folder {
    root: Parts,
    folder: PathBuf,
    /// The cache's record of the folder ([`use_cache_pack`]); `None` for the resources folder.
    marker: Option<PathBuf>,
    /// Changes noted while no pack is in use (lines already in the changes file).
    noted: HashSet<String>,
    /// A build was started for it in this run.
    tried: bool,
}

static FOLDERS: Mutex<Vec<Folder>> = Mutex::new(Vec::new());
static BUILDING: AtomicBool = AtomicBool::new(false);

fn log(line: std::fmt::Arguments) {
    crate::logging::write_log_line_long(line);
    log::info!("{line}");
}

fn disabled() -> bool {
    std::env::var_os("HORIZON_NX_PACK").is_some_and(|v| v == "0")
}

fn mounts() -> std::sync::RwLockReadGuard<'static, Vec<Arc<Mount>>> {
    MOUNTS.read().unwrap_or_else(|e| e.into_inner())
}

fn folders() -> std::sync::MutexGuard<'static, Vec<Folder>> {
    FOLDERS.lock().unwrap_or_else(|e| e.into_inner())
}

fn under(real: &[String], root: &[String]) -> bool {
    real.len() >= root.len() && real[..root.len()] == root[..]
}

/// The folder's absolute parts and its path as the card names it.
fn place(folder: &Path) -> Option<(Parts, PathBuf)> {
    let root = normalize(folder.to_str()?, &super::devoptab::cwd());
    let folder = PathBuf::from(render(&root));
    Some((root, folder))
}

fn register(root: &Parts, folder: &Path, marker: Option<&Path>) {
    let mut all = folders();
    if !all.iter().any(|f| f.root == *root) {
        all.push(Folder {
            root: root.clone(),
            folder: folder.to_path_buf(),
            marker: marker.map(Path::to_path_buf),
            noted: HashSet::new(),
            tried: false,
        });
    }
}

fn mount(m: Mount) {
    let mut all = MOUNTS.write().unwrap_or_else(|e| e.into_inner());
    all.retain(|old| old.root != m.root);
    all.push(Arc::new(m));
}

/// Uses `folder`'s pack from now on, when it has one that still fits it. Call once, before the
/// game reads the folder.
pub fn use_pack(folder: &Path) {
    if disabled() {
        log(format_args!("[Pack] off (HORIZON_NX_PACK=0)"));
        return;
    }
    let Some((root, folder)) = place(folder) else {
        return;
    };
    register(&root, &folder, None);
    let started = Instant::now();
    match open(&root, &folder, None) {
        Ok(m) => {
            let files = m.index.entries.len();
            mount(m);
            log(format_args!(
                "[Pack] {} in use: {files} files and folders, opened in {} ms",
                pack::pack_path(&folder).display(),
                started.elapsed().as_millis()
            ));
        }
        Err(why) => log(format_args!(
            "[Pack] {} not used: {why}; it is built once the game has loaded",
            pack::pack_path(&folder).display()
        )),
    }
}

/// Uses the pack of a folder a cache built (a resource pack overlay), whose own record of what it
/// was built from is the file `marker`: the pack fits while the record is the one it was packed
/// with. Call it each time the folder is about to be read; a folder without a pack gets one from
/// [`build_in_background`], used as soon as it is built.
pub fn use_cache_pack(folder: &Path, marker: &Path) {
    if disabled() {
        return;
    }
    let Some((root, folder)) = place(folder) else {
        return;
    };
    register(&root, &folder, Some(marker));
    let stamp = pack::stamp_of(marker);
    let current = mounts().iter().find(|m| m.root == root).cloned();
    if let Some(m) = current {
        if stamp.is_some() && m.stamp == stamp {
            return;
        }
        // The cache rebuilt the folder: its pack no longer answers for it.
        MOUNTS
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|old| old.root != root);
        if let Some(f) = folders().iter_mut().find(|f| f.root == root) {
            f.tried = false;
        }
        log(format_args!(
            "[Pack] {} was rebuilt: its pack is set aside",
            folder.display()
        ));
    }
    let Some(stamp) = stamp else {
        return;
    };
    let started = Instant::now();
    match open(&root, &folder, Some(&stamp)) {
        Ok(m) => {
            let files = m.index.entries.len();
            mount(m);
            log(format_args!(
                "[Pack] {} in use: {files} files and folders, opened in {} ms",
                pack::pack_path(&folder).display(),
                started.elapsed().as_millis()
            ));
        }
        Err(why) => log(format_args!(
            "[Pack] {} not used: {why}",
            pack::pack_path(&folder).display()
        )),
    }
}

fn open(root: &[String], folder: &Path, stamp: Option<&str>) -> Result<Mount, String> {
    let path = pack::pack_path(folder);
    let c_path = CString::new(path.to_str().unwrap_or_default()).map_err(|e| e.to_string())?;
    let o = super::devoptab::original();
    let mut file = super::devoptab::file_struct();
    let fd = file.as_mut_ptr().cast::<c_void>();
    let r = super::devoptab::reent();
    if unsafe { o.open_r.unwrap()(r, fd, c_path.as_ptr(), libc::O_RDONLY, 0) } == -1 {
        return Err("there is none yet".into());
    }
    let close = |file: &mut Vec<u64>| unsafe {
        o.close_r.unwrap()(r, file.as_mut_ptr().cast());
    };
    let len = unsafe { o.seek_r.unwrap()(r, fd, 0, libc::SEEK_END) };
    let read = |at: u64, n: usize| -> Result<Vec<u8>, String> {
        let mut bytes = vec![0u8; n];
        let mut got = 0u64;
        let rc = unsafe {
            fsFileRead(
                fd,
                at as i64,
                bytes.as_mut_ptr().cast(),
                n as u64,
                0,
                &mut got,
            )
        };
        if rc != 0 || got as usize != n {
            return Err(format!("reading it failed (0x{rc:x})"));
        }
        Ok(bytes)
    };
    let index = (|| {
        if len < 24 + 8 {
            return Err("it is cut short".to_string());
        }
        let (at, n) = pack::footer(&read(len as u64 - 24, 24)?).map_err(|e| e.to_string())?;
        pack::decode(&read(at, n as usize)?).map_err(|e| e.to_string())
    })();
    let index = match index {
        Ok(index) => index,
        Err(e) => {
            close(&mut file);
            return Err(e);
        }
    };
    if let Ok(changes) = std::fs::read_to_string(pack::changes_path(folder)) {
        index.load_changes(&changes);
    }
    let mut now = pack::top_level(folder).map_err(|e| {
        close(&mut file);
        format!("listing the folder failed: {e}")
    })?;
    now.extend(stamp.map(pack::stamp_top));
    if !index.fits(&now) {
        close(&mut file);
        return Err("the folder changed since it was packed".into());
    }
    Ok(Mount {
        root: root.to_vec(),
        folder: folder.to_path_buf(),
        index,
        file,
        stamp: stamp.map(str::to_string),
    })
}

/// Packs the folders with no pack in use, one after another on a background thread. Call it once
/// the game has loaded (and after a reload), so the builds don't slow loading down. The resources
/// folder's pack is used from the next start; a cache folder's as soon as it is built.
pub fn build_in_background() {
    if disabled() || BUILDING.swap(true, Relaxed) {
        return;
    }
    let mounted: Vec<Parts> = mounts().iter().map(|m| m.root.clone()).collect();
    let todo: Vec<(Parts, PathBuf, Option<PathBuf>)> = folders()
        .iter_mut()
        .filter(|f| !f.tried && !mounted.contains(&f.root))
        .map(|f| {
            f.tried = true;
            (f.root.clone(), f.folder.clone(), f.marker.clone())
        })
        .collect();
    if todo.is_empty() {
        BUILDING.store(false, Relaxed);
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("pack-build".into())
        .spawn(move || {
            for (root, folder, marker) in todo {
                build_one(&root, &folder, marker.as_deref());
            }
            BUILDING.store(false, Relaxed);
        });
    if spawned.is_err() {
        BUILDING.store(false, Relaxed);
    }
}

fn build_one(root: &Parts, folder: &Path, marker: Option<&Path>) {
    let stamp = match marker {
        Some(marker) => match pack::stamp_of(marker) {
            Some(stamp) => Some(stamp),
            // The cache has no record of the folder: it isn't finished.
            None => return,
        },
        None => None,
    };
    let started = Instant::now();
    log(format_args!(
        "[Pack] packing {} into {}; {}",
        folder.display(),
        pack::pack_path(folder).display(),
        crate::memory::MallocFigures::now()
    ));
    let result = pack::build(folder, stamp.as_deref(), |files| {
        if files % 4000 == 0 {
            log(format_args!(
                "[Pack] {files} files packed, {:.0} s; {}",
                started.elapsed().as_secs_f64(),
                crate::memory::MallocFigures::now()
            ));
        }
    });
    match result {
        Ok((files, bytes)) => {
            log(format_args!(
                "[Pack] done: {files} files, {:.1} MiB in {:.0} s",
                bytes as f64 / 1048576.0,
                started.elapsed().as_secs_f64()
            ));
            // A cache folder's pack is used right away (changes made meanwhile are in its
            // changes file); the resources folder's from the next start.
            if let Some(marker) = marker
                && pack::stamp_of(marker) == stamp
            {
                match open(root, folder, stamp.as_deref()) {
                    Ok(m) => {
                        mount(m);
                        log(format_args!(
                            "[Pack] {} in use",
                            pack::pack_path(folder).display()
                        ));
                    }
                    Err(why) => log(format_args!(
                        "[Pack] {} not used: {why}",
                        pack::pack_path(folder).display()
                    )),
                }
            }
        }
        Err(e) => log(format_args!(
            "[Pack] packing {} failed: {e}; the folder is used as it is",
            folder.display()
        )),
    }
}

/// The pack in use and `real`'s path in it, when `real` is under a packed folder.
pub(super) fn locate(real: &[String]) -> Option<(Arc<Mount>, String)> {
    let all = mounts();
    let mount = all.iter().find(|m| under(real, &m.root))?;
    Some((mount.clone(), real[mount.root.len()..].join("/")))
}

/// Notes that the game changes `real` (when it's under a packed folder): from now on it comes
/// from the folder.
///
/// Noted without a pack in use too (while the pack is built): the pack built then may hold the
/// file as it was before the change.
pub(super) fn changing(real: &[String]) {
    let (folder, rel, lines) = if let Some((mount, rel)) = locate(real) {
        let Some(lines) = mount.index.note_change(&rel) else {
            return;
        };
        (mount.folder.clone(), rel, lines)
    } else {
        let mut all = folders();
        let Some(f) = all.iter_mut().find(|f| under(real, &f.root)) else {
            return;
        };
        let rel = real[f.root.len()..].join("/");
        if !f.noted.insert(rel.clone()) {
            return;
        }
        let parent = rel.rsplit_once('/').map_or("", |(dir, _)| dir);
        let mut lines = format!("P {rel}\n");
        if !rel.is_empty() {
            lines.push_str(&format!("L {parent}\n"));
        }
        (f.folder.clone(), rel, lines)
    };
    use std::io::Write;
    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(pack::changes_path(&folder))
        .and_then(|mut f| f.write_all(lines.as_bytes()));
    if let Err(e) = appended {
        log(format_args!("[Pack] could not note a change to {rel}: {e}"));
    }
}

/// Fills `st` from the index.
pub(super) unsafe fn fill(st: *mut Stat, entry: &Entry) {
    unsafe {
        core::ptr::write_bytes(st, 0, 1);
        (*st).st_mode = match entry.kind {
            Kind::Dir => libc::S_IFDIR | 0o777,
            _ => libc::S_IFREG | 0o666,
        };
        (*st).st_nlink = 1;
        (*st).st_size = entry.size as _;
        (*st).st_blksize = 512;
        (*st).st_mtim.tv_sec = entry.mtime as _;
        (*st).st_atim.tv_sec = entry.mtime as _;
        (*st).st_ctim.tv_sec = entry.mtime as _;
    }
}

/// Whether the index can say that the folder `dir` (`sdmc:/...`) holds no links' manifest, so
/// looking for one on the card can be skipped.
pub(super) fn no_manifest(dir: &str) -> bool {
    let parts = normalize(dir, "sdmc:/");
    let Some((mount, rel)) = locate(&parts) else {
        return false;
    };
    let manifest = if rel.is_empty() {
        super::MANIFEST.to_string()
    } else {
        format!("{rel}/{}", super::MANIFEST)
    };
    matches!(
        mount.index.find(&rel),
        Found::Indexed(Entry {
            kind: Kind::Dir,
            ..
        })
    ) && mount.index.find(&manifest) == Found::Missing
}

/// A file opened from the pack.
struct Open {
    mount: Arc<Mount>,
    start: u64,
    size: u64,
    at: u64,
    entry: Entry,
}

/// Files open from the pack, by libsysbase's file record (which it gives every call on them).
static OPEN: Mutex<Option<HashMap<usize, Open>>> = Mutex::new(None);
static OPEN_COUNT: AtomicUsize = AtomicUsize::new(0);

fn open_files() -> std::sync::MutexGuard<'static, Option<HashMap<usize, Open>>> {
    OPEN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Opens `entry` (packed at `start`) as the file record `file`.
pub(super) fn open_packed(file: *mut c_void, mount: Arc<Mount>, start: u64, entry: &Entry) {
    let mut open = open_files();
    open.get_or_insert_with(HashMap::new).insert(
        file as usize,
        Open {
            mount,
            start,
            size: entry.size,
            at: 0,
            entry: *entry,
        },
    );
    OPEN_COUNT.fetch_add(1, Relaxed);
}

/// Whether `file` is a record [`open_packed`] filled. A record libnx filled is never one: closing
/// a packed file takes it out before libsysbase can reuse its memory.
pub(super) fn is_packed(file: *mut c_void) -> bool {
    OPEN_COUNT.load(Relaxed) != 0
        && open_files()
            .as_ref()
            .is_some_and(|open| open.contains_key(&(file as usize)))
}

pub(super) fn read(r: *mut c_void, file: *mut c_void, buf: *mut u8, len: usize) -> isize {
    let Some((mount, start, at, size)) = open_files()
        .as_ref()
        .and_then(|open| open.get(&(file as usize)))
        .map(|f| (f.mount.clone(), f.start, f.at, f.size))
    else {
        return super::devoptab::fail(r, libc::EBADF) as isize;
    };
    let n = (size.saturating_sub(at)).min(len as u64);
    if n == 0 {
        return 0;
    }
    let mut got = 0u64;
    let rc = unsafe {
        fsFileRead(
            mount.file.as_ptr().cast_mut().cast(),
            (start + at) as i64,
            buf.cast(),
            n,
            0,
            &mut got,
        )
    };
    if rc != 0 {
        return super::devoptab::fail(r, libc::EIO) as isize;
    }
    if let Some(f) = open_files()
        .as_mut()
        .and_then(|open| open.get_mut(&(file as usize)))
    {
        f.at = at + got;
    }
    got as isize
}

pub(super) fn seek(r: *mut c_void, file: *mut c_void, pos: i64, whence: c_int) -> i64 {
    let mut open = open_files();
    let Some(f) = open
        .as_mut()
        .and_then(|open| open.get_mut(&(file as usize)))
    else {
        return super::devoptab::fail(r, libc::EBADF) as i64;
    };
    let base = match whence {
        libc::SEEK_SET => 0,
        libc::SEEK_CUR => f.at as i64,
        libc::SEEK_END => f.size as i64,
        _ => return super::devoptab::fail(r, libc::EINVAL) as i64,
    };
    match base.checked_add(pos) {
        Some(to) if to >= 0 => {
            f.at = to as u64;
            to
        }
        _ => super::devoptab::fail(r, libc::EINVAL) as i64,
    }
}

pub(super) unsafe fn fstat(file: *mut c_void, st: *mut Stat) -> c_int {
    let entry = open_files()
        .as_ref()
        .and_then(|open| open.get(&(file as usize)))
        .map(|f| f.entry);
    match entry {
        Some(entry) => {
            unsafe { fill(st, &entry) };
            0
        }
        None => -1,
    }
}

pub(super) fn close(file: *mut c_void) {
    if open_files()
        .as_mut()
        .and_then(|open| open.remove(&(file as usize)))
        .is_some()
    {
        OPEN_COUNT.fetch_sub(1, Relaxed);
    }
}

/// What the index says of a path under the packed folder, for the device's calls.
pub(super) enum Answer {
    /// Not under the packed folder, or the index leaves it to the folder.
    Folder,
    Missing,
    /// In the index, and the pack holding it.
    Entry(Entry, Arc<Mount>),
}

pub(super) fn answer(real: &[String]) -> Answer {
    let Some((mount, rel)) = locate(real) else {
        return Answer::Folder;
    };
    match mount.index.find(&rel) {
        Found::Indexed(entry) => Answer::Entry(*entry, mount.clone()),
        Found::Missing => Answer::Missing,
        Found::Folder => Answer::Folder,
    }
}

/// The names and entries of the folder `real` when the index can list it.
pub(super) fn listing(real: &[String]) -> Option<Vec<(String, Entry)>> {
    let (mount, rel) = locate(real)?;
    let names = mount.index.list(&rel)?;
    Some(
        names
            .iter()
            .filter_map(|name| {
                if name == super::MANIFEST {
                    return None;
                }
                let child = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                match mount.index.find(&child) {
                    Found::Indexed(entry) => Some((name.clone(), *entry)),
                    _ => None,
                }
            })
            .collect(),
    )
}

/// The `Kind::Packed` offset of an entry, if it's packed.
pub(super) fn packed_at(entry: &Entry) -> Option<u64> {
    match entry.kind {
        Kind::Packed(at) => Some(at),
        _ => None,
    }
}
