//! Safe wrapper around `SftpHandle` for direct SFTP access.
//!
//! Falls back to `None` if the SSH plugin hasn't registered an SFTP vtable
//! for the given session (e.g., old SSH plugin version).

use conch_plugin_sdk::sftp::SftpHandle;
use conch_plugin_sdk::HostApi;

use crate::FileEntry;

/// A safe SFTP handle that automatically releases on drop.
pub struct SftpAccess {
    handle: SftpHandle,
}

impl SftpAccess {
    /// Try to acquire a direct SFTP handle for the given session.
    /// Returns `None` if not available (SSH plugin doesn't support vtable).
    pub fn acquire(api: &HostApi, session_id: u64) -> Option<Self> {
        let handle = (api.acquire_sftp)(session_id);
        if handle.vtable.is_null() || handle.ctx.is_null() {
            return None;
        }
        Some(Self { handle })
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>, String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.list_dir)(
            self.handle.ctx,
            path.as_ptr(),
            path.len(),
        );

        if !result.error.is_null() && result.error_len > 0 {
            let err = unsafe {
                let slice = std::slice::from_raw_parts(result.error, result.error_len);
                String::from_utf8_lossy(slice).to_string()
            };
            (vtable.free_buf)(result.error, result.error_len);
            return Err(err);
        }

        let mut entries = Vec::with_capacity(result.count);
        if !result.entries.is_null() && result.count > 0 {
            let dir_entries = unsafe {
                std::slice::from_raw_parts(result.entries, result.count)
            };
            for de in dir_entries {
                let name = if !de.name.is_null() && de.name_len > 0 {
                    unsafe {
                        let slice = std::slice::from_raw_parts(de.name, de.name_len);
                        String::from_utf8_lossy(slice).to_string()
                    }
                } else {
                    String::new()
                };
                entries.push(FileEntry {
                    name,
                    is_dir: de.is_dir,
                    size: de.size,
                    modified: if de.mtime > 0 { Some(de.mtime) } else { None },
                });
            }
        }

        // Free the list result (entries + name buffers).
        let mut result = result;
        (vtable.free_list)(&mut result);

        crate::local::sort_entries(&mut entries);
        Ok(entries)
    }

    pub fn read_chunk(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>, String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.read_chunk)(
            self.handle.ctx,
            path.as_ptr(),
            path.len(),
            offset,
            length,
        );

        if !result.error.is_null() && result.error_len > 0 {
            let err = unsafe {
                let slice = std::slice::from_raw_parts(result.error, result.error_len);
                String::from_utf8_lossy(slice).to_string()
            };
            (vtable.free_buf)(result.error, result.error_len);
            return Err(err);
        }

        let data = if !result.data.is_null() && result.len > 0 {
            unsafe {
                std::slice::from_raw_parts(result.data, result.len).to_vec()
            }
        } else {
            Vec::new()
        };

        // Free the data buffer.
        if !result.data.is_null() {
            (vtable.free_buf)(result.data, result.len);
        }

        Ok(data)
    }

    pub fn write_file(&self, path: &str, data: &[u8]) -> Result<(), String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.write_file)(
            self.handle.ctx,
            path.as_ptr(),
            path.len(),
            data.as_ptr(),
            data.len(),
        );
        check_simple(vtable, result)
    }

    pub fn write_at(&self, path: &str, data: &[u8], offset: u64, truncate: bool) -> Result<(), String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.write_at)(
            self.handle.ctx,
            path.as_ptr(),
            path.len(),
            data.as_ptr(),
            data.len(),
            offset,
            truncate,
        );
        check_simple(vtable, result)
    }

    pub fn mkdir(&self, path: &str) -> Result<(), String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.mkdir)(
            self.handle.ctx,
            path.as_ptr(),
            path.len(),
        );
        check_simple(vtable, result)
    }

    #[allow(dead_code)]
    pub fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.rename)(
            self.handle.ctx,
            from.as_ptr(),
            from.len(),
            to.as_ptr(),
            to.len(),
        );
        check_simple(vtable, result)
    }

    #[allow(dead_code)]
    pub fn delete(&self, path: &str, is_dir: bool) -> Result<(), String> {
        let vtable = unsafe { &*self.handle.vtable };
        let result = (vtable.delete)(
            self.handle.ctx,
            path.as_ptr(),
            path.len(),
            is_dir,
        );
        check_simple(vtable, result)
    }
}

impl Drop for SftpAccess {
    fn drop(&mut self) {
        if !self.handle.vtable.is_null() && !self.handle.ctx.is_null() {
            let vtable = unsafe { &*self.handle.vtable };
            (vtable.release)(self.handle.ctx);
        }
    }
}

fn check_simple(
    vtable: &conch_plugin_sdk::sftp::SftpVtable,
    result: conch_plugin_sdk::sftp::SftpSimpleResult,
) -> Result<(), String> {
    if result.ok {
        return Ok(());
    }

    if !result.error.is_null() && result.error_len > 0 {
        let err = unsafe {
            let slice = std::slice::from_raw_parts(result.error, result.error_len);
            String::from_utf8_lossy(slice).to_string()
        };
        (vtable.free_buf)(result.error, result.error_len);
        Err(err)
    } else {
        Err("unknown error".to_string())
    }
}
