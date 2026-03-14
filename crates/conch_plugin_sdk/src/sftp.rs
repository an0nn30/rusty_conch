//! SFTP vtable types for direct cross-plugin SFTP access.
//!
//! The SSH plugin registers an `SftpVtable` + opaque context for each session.
//! Other plugins (e.g., the file explorer) acquire an `SftpHandle` and call
//! SFTP operations directly — no JSON serialization, no base64, no bus routing.

use std::ffi::c_void;

/// Result of an SFTP read-chunk operation.
#[repr(C)]
pub struct SftpReadResult {
    /// Pointer to the data buffer (owned by callee, freed via `SftpVtable::free_buf`).
    pub data: *mut u8,
    /// Length of the data.
    pub len: usize,
    /// Error message (null on success). Freed via `SftpVtable::free_buf`.
    pub error: *mut u8,
    pub error_len: usize,
}

/// A single directory entry returned by `list_dir`.
#[repr(C)]
pub struct SftpDirEntry {
    pub name: *mut u8,
    pub name_len: usize,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: u64,
}

/// Result of an SFTP directory listing.
#[repr(C)]
pub struct SftpListResult {
    pub entries: *mut SftpDirEntry,
    pub count: usize,
    pub error: *mut u8,
    pub error_len: usize,
}

/// Result of an SFTP write/mkdir/rename/delete operation.
#[repr(C)]
pub struct SftpSimpleResult {
    pub ok: bool,
    pub error: *mut u8,
    pub error_len: usize,
}

/// Vtable for direct SFTP operations across plugin boundaries.
///
/// All functions take an opaque `ctx` pointer as the first argument.
/// All calls are blocking — the callee runs async SFTP internally.
#[repr(C)]
pub struct SftpVtable {
    pub list_dir: extern "C" fn(ctx: *mut c_void, path: *const u8, path_len: usize) -> SftpListResult,
    pub read_chunk: extern "C" fn(ctx: *mut c_void, path: *const u8, path_len: usize, offset: u64, length: u64) -> SftpReadResult,
    pub write_file: extern "C" fn(ctx: *mut c_void, path: *const u8, path_len: usize, data: *const u8, data_len: usize) -> SftpSimpleResult,
    pub mkdir: extern "C" fn(ctx: *mut c_void, path: *const u8, path_len: usize) -> SftpSimpleResult,
    pub rename: extern "C" fn(ctx: *mut c_void, from: *const u8, from_len: usize, to: *const u8, to_len: usize) -> SftpSimpleResult,
    pub delete: extern "C" fn(ctx: *mut c_void, path: *const u8, path_len: usize, is_dir: bool) -> SftpSimpleResult,
    /// Free a buffer returned by `read_chunk`, `list_dir`, or error messages.
    pub free_buf: extern "C" fn(ptr: *mut u8, len: usize),
    /// Free a `SftpListResult`'s entries array and all contained strings.
    pub free_list: extern "C" fn(result: *mut SftpListResult),
    /// Increment the reference count. The handle stays valid until `release` is called.
    pub retain: extern "C" fn(ctx: *mut c_void),
    /// Decrement the reference count. When it hits zero, the context is freed.
    pub release: extern "C" fn(ctx: *mut c_void),
}

// SAFETY: SftpVtable contains only function pointers (all thread-safe).
unsafe impl Send for SftpVtable {}
unsafe impl Sync for SftpVtable {}

/// An acquired SFTP handle — vtable + opaque context pointer.
#[repr(C)]
pub struct SftpHandle {
    pub vtable: *const SftpVtable,
    pub ctx: *mut c_void,
}

// SAFETY: The vtable functions are thread-safe by contract, and the ctx
// pointer is only accessed through the vtable.
unsafe impl Send for SftpHandle {}
unsafe impl Sync for SftpHandle {}
