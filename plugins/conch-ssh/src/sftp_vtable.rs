//! SFTP vtable implementation — provides direct SFTP access to other plugins.
//!
//! `SftpContext` wraps a pointer to the `SshBackendState` (which owns the SSH
//! handle) and the tokio runtime handle. The vtable functions cast the opaque
//! `ctx` pointer back to `SftpContext` and call async SFTP operations via
//! `rt.block_on()`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use conch_plugin_sdk::sftp::{
    SftpDirEntry, SftpListResult, SftpReadResult, SftpSimpleResult, SftpVtable,
};

use crate::session_backend::SshBackendState;
use crate::sftp;

/// Opaque context shared across plugin boundaries via retain/release.
///
/// Holds a raw pointer to the `SshBackendState` which is Box-heap-allocated
/// and lives for the entire session. The pointer is valid as long as the SSH
/// session exists (the SSH plugin won't free it while the vtable is registered).
pub struct SftpContext {
    /// Raw pointer to the SshBackendState that owns the SSH handle.
    /// SAFETY: The SshBackendState is heap-allocated in a Box and stored in
    /// the SSH plugin's sessions HashMap. It outlives the vtable registration.
    backend: *const SshBackendState,
    pub rt: tokio::runtime::Handle,
    ref_count: AtomicUsize,
}

// SAFETY: The backend pointer is only accessed through the vtable functions
// which call the thread-safe SSH handle methods.
unsafe impl Send for SftpContext {}
unsafe impl Sync for SftpContext {}

impl SftpContext {
    /// Create a new SftpContext. The returned pointer has ref_count = 1.
    ///
    /// # Safety
    /// `backend` must point to a valid `SshBackendState` that outlives all
    /// users of this context.
    pub unsafe fn new(
        backend: *const SshBackendState,
        rt: tokio::runtime::Handle,
    ) -> *mut Self {
        let ctx = Box::new(Self {
            backend,
            rt,
            ref_count: AtomicUsize::new(1),
        });
        Box::into_raw(ctx)
    }

    fn ssh_handle(&self) -> Result<&russh::client::Handle<crate::SshHandler>, String> {
        // SAFETY: The backend pointer is valid for the lifetime of the session.
        let backend = unsafe { &*self.backend };
        backend
            .ssh_handle()
            .ok_or_else(|| "SSH session not connected".to_string())
    }
}

/// The static vtable — all function pointers for SFTP operations.
pub static SFTP_VTABLE: SftpVtable = SftpVtable {
    list_dir: sftp_list_dir,
    read_chunk: sftp_read_chunk,
    write_file: sftp_write_file,
    write_at: sftp_write_at,
    mkdir: sftp_mkdir,
    rename: sftp_rename,
    delete: sftp_delete,
    free_buf: sftp_free_buf,
    free_list: sftp_free_list,
    retain: sftp_retain,
    release: sftp_release,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an error string into a heap-allocated buffer, returning (ptr, len).
fn error_buf(msg: &str) -> (*mut u8, usize) {
    let bytes = msg.as_bytes().to_vec();
    let len = bytes.len();
    let mut boxed = bytes.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (ptr, len)
}

/// Read a `(ptr, len)` pair as a `&str`.
unsafe fn ptr_to_str<'a>(ptr: *const u8, len: usize) -> &'a str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(slice).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Vtable function implementations
// ---------------------------------------------------------------------------

extern "C" fn sftp_list_dir(
    ctx: *mut c_void,
    path: *const u8,
    path_len: usize,
) -> SftpListResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let path_str = unsafe { ptr_to_str(path, path_len) };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpListResult {
                entries: std::ptr::null_mut(),
                count: 0,
                error: ep,
                error_len: el,
            };
        }
    };

    match context.rt.block_on(sftp::list_dir(ssh_handle, path_str)) {
        Ok(val) => {
            let entries_val = val["entries"].as_array();
            let Some(arr) = entries_val else {
                let (ep, el) = error_buf("missing entries array");
                return SftpListResult {
                    entries: std::ptr::null_mut(),
                    count: 0,
                    error: ep,
                    error_len: el,
                };
            };

            let mut dir_entries: Vec<SftpDirEntry> = arr
                .iter()
                .map(|e| {
                    let name = e["name"].as_str().unwrap_or("").as_bytes().to_vec();
                    let name_len = name.len();
                    let mut name_box = name.into_boxed_slice();
                    let name_ptr = name_box.as_mut_ptr();
                    std::mem::forget(name_box);

                    SftpDirEntry {
                        name: name_ptr,
                        name_len,
                        is_dir: e["is_dir"].as_bool().unwrap_or(false),
                        size: e["size"].as_u64().unwrap_or(0),
                        mtime: e["mtime"].as_u64().unwrap_or(0),
                    }
                })
                .collect();

            let count = dir_entries.len();
            let ptr = dir_entries.as_mut_ptr();
            std::mem::forget(dir_entries);

            SftpListResult {
                entries: ptr,
                count,
                error: std::ptr::null_mut(),
                error_len: 0,
            }
        }
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpListResult {
                entries: std::ptr::null_mut(),
                count: 0,
                error: ep,
                error_len: el,
            }
        }
    }
}

extern "C" fn sftp_read_chunk(
    ctx: *mut c_void,
    path: *const u8,
    path_len: usize,
    offset: u64,
    length: u64,
) -> SftpReadResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let path_str = unsafe { ptr_to_str(path, path_len) };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpReadResult {
                data: std::ptr::null_mut(),
                len: 0,
                error: ep,
                error_len: el,
            };
        }
    };

    // Direct SFTP read — bypasses base64 encoding.
    let result = context.rt.block_on(async {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let sftp_session = sftp::open_sftp_pub(ssh_handle).await?;
        let mut file = sftp_session
            .open(path_str)
            .await
            .map_err(|e| format!("open failed: {e}"))?;

        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|e| format!("seek failed: {e}"))?;
        }

        let cap = (length as usize).min(1024 * 1024);
        let mut buf = vec![0u8; cap];
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        buf.truncate(n);
        Ok::<Vec<u8>, String>(buf)
    });

    match result {
        Ok(data) => {
            let len = data.len();
            let mut boxed = data.into_boxed_slice();
            let ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed);
            SftpReadResult {
                data: ptr,
                len,
                error: std::ptr::null_mut(),
                error_len: 0,
            }
        }
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpReadResult {
                data: std::ptr::null_mut(),
                len: 0,
                error: ep,
                error_len: el,
            }
        }
    }
}

extern "C" fn sftp_write_file(
    ctx: *mut c_void,
    path: *const u8,
    path_len: usize,
    data: *const u8,
    data_len: usize,
) -> SftpSimpleResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let path_str = unsafe { ptr_to_str(path, path_len) };
    let data_slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            };
        }
    };

    let result = context.rt.block_on(async {
        use tokio::io::AsyncWriteExt;

        let sftp_session = sftp::open_sftp_pub(ssh_handle).await?;
        let mut file = sftp_session
            .create(path_str)
            .await
            .map_err(|e| format!("create failed: {e}"))?;
        file.write_all(data_slice)
            .await
            .map_err(|e| format!("write failed: {e}"))?;
        Ok::<(), String>(())
    });

    match result {
        Ok(()) => SftpSimpleResult {
            ok: true,
            error: std::ptr::null_mut(),
            error_len: 0,
        },
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            }
        }
    }
}

extern "C" fn sftp_write_at(
    ctx: *mut c_void,
    path: *const u8,
    path_len: usize,
    data: *const u8,
    data_len: usize,
    offset: u64,
    truncate: bool,
) -> SftpSimpleResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let path_str = unsafe { ptr_to_str(path, path_len) };
    let data_slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            };
        }
    };

    let result = context.rt.block_on(async {
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};

        let sftp_session = sftp::open_sftp_pub(ssh_handle).await?;
        let mut file = if truncate {
            sftp_session
                .create(path_str)
                .await
                .map_err(|e| format!("create failed: {e}"))?
        } else {
            use russh_sftp::protocol::OpenFlags;
            sftp_session
                .open_with_flags(
                    path_str,
                    OpenFlags::CREATE | OpenFlags::WRITE,
                )
                .await
                .map_err(|e| format!("open failed: {e}"))?
        };

        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|e| format!("seek failed: {e}"))?;
        }

        file.write_all(data_slice)
            .await
            .map_err(|e| format!("write failed: {e}"))?;
        Ok::<(), String>(())
    });

    match result {
        Ok(()) => SftpSimpleResult {
            ok: true,
            error: std::ptr::null_mut(),
            error_len: 0,
        },
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            }
        }
    }
}

extern "C" fn sftp_mkdir(
    ctx: *mut c_void,
    path: *const u8,
    path_len: usize,
) -> SftpSimpleResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let path_str = unsafe { ptr_to_str(path, path_len) };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            };
        }
    };

    match context.rt.block_on(sftp::mkdir(ssh_handle, path_str)) {
        Ok(_) => SftpSimpleResult {
            ok: true,
            error: std::ptr::null_mut(),
            error_len: 0,
        },
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            }
        }
    }
}

extern "C" fn sftp_rename(
    ctx: *mut c_void,
    from: *const u8,
    from_len: usize,
    to: *const u8,
    to_len: usize,
) -> SftpSimpleResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let from_str = unsafe { ptr_to_str(from, from_len) };
    let to_str = unsafe { ptr_to_str(to, to_len) };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            };
        }
    };

    match context.rt.block_on(sftp::rename(ssh_handle, from_str, to_str)) {
        Ok(_) => SftpSimpleResult {
            ok: true,
            error: std::ptr::null_mut(),
            error_len: 0,
        },
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            }
        }
    }
}

extern "C" fn sftp_delete(
    ctx: *mut c_void,
    path: *const u8,
    path_len: usize,
    is_dir: bool,
) -> SftpSimpleResult {
    let context = unsafe { &*(ctx as *const SftpContext) };
    let path_str = unsafe { ptr_to_str(path, path_len) };

    let ssh_handle = match context.ssh_handle() {
        Ok(h) => h,
        Err(e) => {
            let (ep, el) = error_buf(&e);
            return SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            };
        }
    };

    let result = if is_dir {
        context.rt.block_on(sftp::remove_dir(ssh_handle, path_str))
    } else {
        context.rt.block_on(sftp::remove_file(ssh_handle, path_str))
    };

    match result {
        Ok(_) => SftpSimpleResult {
            ok: true,
            error: std::ptr::null_mut(),
            error_len: 0,
        },
        Err(e) => {
            let (ep, el) = error_buf(&e);
            SftpSimpleResult {
                ok: false,
                error: ep,
                error_len: el,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Memory management
// ---------------------------------------------------------------------------

extern "C" fn sftp_free_buf(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            drop(Box::from_raw(std::slice::from_raw_parts_mut(ptr, len)));
        }
    }
}

extern "C" fn sftp_free_list(result: *mut SftpListResult) {
    if result.is_null() {
        return;
    }
    let r = unsafe { &mut *result };

    // Free each entry's name buffer.
    if !r.entries.is_null() && r.count > 0 {
        let entries = unsafe { std::slice::from_raw_parts_mut(r.entries, r.count) };
        for entry in entries.iter() {
            if !entry.name.is_null() && entry.name_len > 0 {
                unsafe {
                    drop(Box::from_raw(std::slice::from_raw_parts_mut(
                        entry.name,
                        entry.name_len,
                    )));
                }
            }
        }
        // Free the entries array itself.
        unsafe {
            drop(Vec::from_raw_parts(r.entries, r.count, r.count));
        }
    }

    // Free error buffer if present.
    if !r.error.is_null() && r.error_len > 0 {
        unsafe {
            drop(Box::from_raw(std::slice::from_raw_parts_mut(
                r.error,
                r.error_len,
            )));
        }
    }
}

// ---------------------------------------------------------------------------
// Reference counting
// ---------------------------------------------------------------------------

extern "C" fn sftp_retain(ctx: *mut c_void) {
    if !ctx.is_null() {
        let context = unsafe { &*(ctx as *const SftpContext) };
        context.ref_count.fetch_add(1, Ordering::AcqRel);
    }
}

extern "C" fn sftp_release(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let context = unsafe { &*(ctx as *const SftpContext) };
    let prev = context.ref_count.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        // Last reference — free the context.
        unsafe {
            drop(Box::from_raw(ctx as *mut SftpContext));
        }
        log::debug!("SftpContext freed (ref_count dropped to 0)");
    }
}
