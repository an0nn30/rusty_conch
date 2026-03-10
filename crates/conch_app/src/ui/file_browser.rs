//! File browser state for the left sidebar.

use std::path::PathBuf;

/// Which pane is active in the file browser when focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileBrowserPane {
    #[default]
    Local,
    Remote,
    Local2,
}

/// State for the file browser panel.
#[derive(Debug, Clone)]
pub struct FileBrowserState {
    /// Whether the file browser has keyboard focus.
    pub focused: bool,
    /// Which pane (local/remote) is active for keyboard navigation.
    pub active_pane: FileBrowserPane,
    pub local_path: PathBuf,
    pub local_entries: Vec<FileListEntry>,
    pub local_path_edit: String,
    pub local_back_stack: Vec<PathBuf>,
    pub local_forward_stack: Vec<PathBuf>,
    pub local_selected: Option<usize>,
    pub remote_path: Option<PathBuf>,
    pub remote_entries: Vec<FileListEntry>,
    pub remote_path_edit: String,
    pub remote_back_stack: Vec<PathBuf>,
    pub remote_forward_stack: Vec<PathBuf>,
    pub remote_selected: Option<usize>,
    /// Second local pane (shown when no remote session is active).
    pub local2_path: PathBuf,
    pub local2_entries: Vec<FileListEntry>,
    pub local2_path_edit: String,
    pub local2_back_stack: Vec<PathBuf>,
    pub local2_forward_stack: Vec<PathBuf>,
    pub local2_selected: Option<usize>,
}

/// A single file or directory entry.
#[derive(Debug, Clone)]
pub struct FileListEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<u64>,
}

impl From<conch_session::FileEntry> for FileListEntry {
    fn from(e: conch_session::FileEntry) -> Self {
        Self {
            name: e.name,
            path: e.path,
            is_dir: e.is_dir,
            size: e.size,
            modified: e.modified,
        }
    }
}

impl Default for FileBrowserState {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let local_path_edit = home.to_string_lossy().into_owned();
        let local2_path_edit = local_path_edit.clone();
        Self {
            focused: false,
            active_pane: FileBrowserPane::default(),
            local_path: home.clone(),
            local_entries: Vec::new(),
            local_path_edit,
            local_back_stack: Vec::new(),
            local_forward_stack: Vec::new(),
            local_selected: None,
            remote_path: None,
            remote_entries: Vec::new(),
            remote_path_edit: String::new(),
            remote_back_stack: Vec::new(),
            remote_forward_stack: Vec::new(),
            remote_selected: None,
            local2_path: home,
            local2_entries: Vec::new(),
            local2_path_edit,
            local2_back_stack: Vec::new(),
            local2_forward_stack: Vec::new(),
            local2_selected: None,
        }
    }
}

/// Format a byte count as a human-readable size string.
pub fn display_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Recursively copy a directory and its contents.
/// Skips symlinks to avoid infinite recursion from cyclic links.
// TODO: prompt for confirmation before overwriting existing files.
pub fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &dest_path)?;
        }
        // symlinks are silently skipped
    }
    Ok(())
}

/// Format an optional UNIX timestamp as a short date string.
pub fn format_modified(timestamp: Option<u64>) -> String {
    match timestamp {
        Some(ts) => {
            let dt = chrono::DateTime::from_timestamp(ts as i64, 0);
            match dt {
                Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
                None => "—".to_string(),
            }
        }
        None => "—".to_string(),
    }
}
