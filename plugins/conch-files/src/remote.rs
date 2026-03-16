//! Remote filesystem operations via SFTP (queried through conch-ssh RPC).

use std::ffi::CString;

use conch_plugin_sdk::HostApi;
use serde_json::json;

use crate::FileEntry;

/// Query the SSH plugin's `list_dir` service for a remote directory listing.
pub fn list_dir(api: &HostApi, session_id: u64, path: &str) -> Result<Vec<FileEntry>, String> {
    let target = CString::new("SSH Manager").unwrap();
    let method = CString::new("list_dir").unwrap();
    let args = json!({ "session_id": session_id, "path": path });
    let args_str = serde_json::to_string(&args).unwrap();

    let result_ptr = (api.query_plugin)(
        target.as_ptr(),
        method.as_ptr(),
        args_str.as_ptr() as *const _,
        args_str.len(),
    );

    if result_ptr.is_null() {
        return Err("query_plugin returned null".to_string());
    }

    let result_cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
    let result_str = result_cstr.to_string_lossy().to_string();
    (api.free_string)(result_ptr);

    let val: serde_json::Value =
        serde_json::from_str(&result_str).map_err(|e| format!("JSON parse error: {e}"))?;

    if val["status"] != "ok" {
        return Err(
            val["message"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string(),
        );
    }

    let entries_val = val["entries"]
        .as_array()
        .ok_or("missing entries array")?;

    let mut entries: Vec<FileEntry> = entries_val
        .iter()
        .map(|e| FileEntry {
            name: e["name"].as_str().unwrap_or("").to_string(),
            is_dir: e["is_dir"].as_bool().unwrap_or(false),
            size: e["size"].as_u64().unwrap_or(0),
            modified: e["mtime"].as_u64().filter(|&v| v > 0),
        })
        .collect();

    crate::local::sort_entries(&mut entries);
    Ok(entries)
}

/// Resolve a path to its canonical absolute form on the remote.
pub fn realpath(api: &HostApi, session_id: u64, path: &str) -> Result<String, String> {
    let target = CString::new("SSH Manager").unwrap();
    let method = CString::new("realpath").unwrap();
    let args = json!({ "session_id": session_id, "path": path });
    let args_str = serde_json::to_string(&args).unwrap();

    let result_ptr = (api.query_plugin)(
        target.as_ptr(),
        method.as_ptr(),
        args_str.as_ptr() as *const _,
        args_str.len(),
    );

    if result_ptr.is_null() {
        return Err("query_plugin returned null".to_string());
    }

    let result_cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
    let result_str = result_cstr.to_string_lossy().to_string();
    (api.free_string)(result_ptr);

    let val: serde_json::Value =
        serde_json::from_str(&result_str).map_err(|e| format!("JSON parse error: {e}"))?;

    if val["status"] != "ok" {
        return Err(val["message"].as_str().unwrap_or("unknown error").to_string());
    }

    Ok(val["path"].as_str().unwrap_or(".").to_string())
}

/// Create a directory on the remote via SFTP.
pub fn mkdir(api: &HostApi, session_id: u64, path: &str) -> Result<(), String> {
    query_simple(api, session_id, "mkdir", json!({ "session_id": session_id, "path": path }))
}

/// Rename a file or directory on the remote via SFTP.
pub fn rename(api: &HostApi, session_id: u64, from: &str, to: &str) -> Result<(), String> {
    query_simple(api, session_id, "rename", json!({ "session_id": session_id, "from": from, "to": to }))
}

/// Delete a file or directory on the remote via SFTP.
pub fn delete(api: &HostApi, session_id: u64, path: &str, is_dir: bool) -> Result<(), String> {
    query_simple(api, session_id, "delete", json!({ "session_id": session_id, "path": path, "is_dir": is_dir }))
}

/// Read a file from the remote via SFTP. Returns raw bytes.
pub fn read_file(api: &HostApi, session_id: u64, path: &str) -> Result<Vec<u8>, String> {
    read_file_with_progress(api, session_id, path, 0, None)
}

/// Read a file from the remote via SFTP with optional progress reporting.
///
/// `file_size` is the expected total size (for progress calculation). Pass 0 if unknown.
/// `on_progress` is called after each chunk with `(bytes_so_far, file_size)`.
pub fn read_file_with_progress(
    api: &HostApi,
    session_id: u64,
    path: &str,
    file_size: u64,
    on_progress: Option<&dyn Fn(u64, u64)>,
) -> Result<Vec<u8>, String> {
    use base64::Engine;

    // Read in 1MB chunks.
    let mut all_data = Vec::new();
    let mut offset: u64 = 0;
    let chunk_size: u64 = 1024 * 1024;

    loop {
        let target = CString::new("SSH Manager").unwrap();
        let method = CString::new("read_file").unwrap();
        let args = json!({
            "session_id": session_id,
            "path": path,
            "offset": offset,
            "length": chunk_size,
        });
        let args_str = serde_json::to_string(&args).unwrap();

        let result_ptr = (api.query_plugin)(
            target.as_ptr(),
            method.as_ptr(),
            args_str.as_ptr() as *const _,
            args_str.len(),
        );

        if result_ptr.is_null() {
            return Err("query_plugin returned null".to_string());
        }

        let result_cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
        let result_str = result_cstr.to_string_lossy().to_string();
        (api.free_string)(result_ptr);

        let val: serde_json::Value =
            serde_json::from_str(&result_str).map_err(|e| format!("JSON parse error: {e}"))?;

        if val["status"] != "ok" {
            return Err(val["message"].as_str().unwrap_or("unknown error").to_string());
        }

        let data_b64 = val["data"].as_str().unwrap_or("");
        let chunk = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| format!("base64 decode error: {e}"))?;

        let bytes_read = val["bytes_read"].as_u64().unwrap_or(0);
        all_data.extend_from_slice(&chunk);

        if let Some(cb) = on_progress {
            cb(all_data.len() as u64, file_size);
        }

        if bytes_read < chunk_size {
            break;
        }
        offset += bytes_read;
    }

    Ok(all_data)
}

/// Write a file to the remote via SFTP.
pub fn write_file(api: &HostApi, session_id: u64, path: &str, data: &[u8]) -> Result<(), String> {
    use base64::Engine;

    let data_b64 = base64::engine::general_purpose::STANDARD.encode(data);
    query_simple(api, session_id, "write_file", json!({
        "session_id": session_id,
        "path": path,
        "data": data_b64,
    }))
}

fn query_simple(api: &HostApi, _session_id: u64, method: &str, args: serde_json::Value) -> Result<(), String> {
    let target = CString::new("SSH Manager").unwrap();
    let method_c = CString::new(method).unwrap();
    let args_str = serde_json::to_string(&args).unwrap();

    let result_ptr = (api.query_plugin)(
        target.as_ptr(),
        method_c.as_ptr(),
        args_str.as_ptr() as *const _,
        args_str.len(),
    );

    if result_ptr.is_null() {
        return Err("query_plugin returned null".to_string());
    }

    let result_cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
    let result_str = result_cstr.to_string_lossy().to_string();
    (api.free_string)(result_ptr);

    let val: serde_json::Value =
        serde_json::from_str(&result_str).map_err(|e| format!("JSON parse error: {e}"))?;

    if val["status"] != "ok" {
        Err(val["message"].as_str().unwrap_or("unknown error").to_string())
    } else {
        Ok(())
    }
}
