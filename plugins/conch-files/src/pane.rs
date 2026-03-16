//! A single file browser pane — reusable for both local and remote views.

use crate::format;
use crate::local;
use crate::remote;
use crate::FileEntry;
use conch_plugin_sdk::widgets::{
    ContextMenuItem, TableCell, TableColumn, TableRow, TextStyle, ToolbarItem, Widget, WidgetEvent,
};
use conch_plugin_sdk::HostApi;

// Column IDs.
const COL_NAME: &str = "name";
const COL_EXT: &str = "ext";
const COL_SIZE: &str = "size";
const COL_MODIFIED: &str = "modified";

/// Serializable column visibility and display settings for persistence.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnVisibility {
    pub ext: bool,
    pub size: bool,
    pub modified: bool,
    #[serde(default)]
    pub show_hidden: bool,
}

/// Whether this pane browses locally or via SFTP.
#[derive(Clone)]
pub enum PaneMode {
    Local,
    Remote { session_id: u64, host: String, user: String },
}

/// A single file browser pane with its own navigation, entries, and selection.
pub struct Pane {
    /// Unique prefix for widget IDs (e.g. "local" or "remote").
    pub prefix: String,
    pub mode: PaneMode,

    // Navigation.
    pub current_path: String,
    pub path_input: String,
    pub back_stack: Vec<String>,
    pub forward_stack: Vec<String>,

    // Content.
    pub entries: Vec<FileEntry>,
    pub selected_row: Option<String>,
    pub sort_column: String,
    pub sort_ascending: bool,

    // Column visibility.
    pub col_ext_visible: bool,
    pub col_size_visible: bool,
    pub col_modified_visible: bool,

    // Filtering.
    pub show_hidden: bool,

    // State.
    pub error: Option<String>,
    pub dirty: bool,
    pub home_path: String,
    /// The local home directory — preserved across mode switches.
    local_home: String,
}

impl Pane {
    pub fn new_local(prefix: &str) -> Self {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        let mut pane = Self {
            prefix: prefix.to_string(),
            mode: PaneMode::Local,
            current_path: home.clone(),
            path_input: home.clone(),
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            entries: Vec::new(),
            selected_row: None,
            sort_column: COL_NAME.to_string(),
            sort_ascending: true,
            col_ext_visible: false,
            col_size_visible: true,
            col_modified_visible: false,
            show_hidden: false,
            error: None,
            dirty: true,
            home_path: home.clone(),
            local_home: home,
        };
        pane.load_entries(None);
        pane
    }

    // -------------------------------------------------------------------
    // Navigation
    // -------------------------------------------------------------------

    pub fn navigate_to(&mut self, path: &str, api: Option<&HostApi>) {
        // Resolve "~" to the actual home directory for remote SFTP.
        let resolved = if matches!(self.mode, PaneMode::Remote { .. }) && (path == "~" || path.starts_with("~/")) {
            path.replacen("~", &self.home_path, 1)
        } else {
            path.to_string()
        };
        self.back_stack.push(self.current_path.clone());
        self.forward_stack.clear();
        self.current_path = resolved.clone();
        self.path_input = resolved;
        self.selected_row = None;
        self.load_entries(api);
    }

    pub fn go_back(&mut self, api: Option<&HostApi>) {
        if let Some(prev) = self.back_stack.pop() {
            self.forward_stack.push(self.current_path.clone());
            self.current_path = prev.clone();
            self.path_input = prev;
            self.selected_row = None;
            self.load_entries(api);
        }
    }

    pub fn go_forward(&mut self, api: Option<&HostApi>) {
        if let Some(next) = self.forward_stack.pop() {
            self.back_stack.push(self.current_path.clone());
            self.current_path = next.clone();
            self.path_input = next;
            self.selected_row = None;
            self.load_entries(api);
        }
    }

    pub fn go_home(&mut self, api: Option<&HostApi>) {
        let home = self.home_path.clone();
        self.navigate_to(&home, api);
    }

    pub fn refresh(&mut self, api: Option<&HostApi>) {
        self.selected_row = None;
        self.load_entries(api);
    }

    pub fn load_entries(&mut self, api: Option<&HostApi>) {
        self.error = None;
        let result = match &self.mode {
            PaneMode::Local => local::list_dir(&self.current_path),
            PaneMode::Remote { session_id, .. } => {
                if let Some(api) = api {
                    remote::list_dir(api, *session_id, &self.current_path)
                } else {
                    Err("no API available for remote listing".to_string())
                }
            }
        };

        match result {
            Ok(entries) => {
                self.entries = entries;
                self.apply_sort();
            }
            Err(e) => {
                self.error = Some(e);
                self.entries.clear();
            }
        }
        self.dirty = true;
    }

    // -------------------------------------------------------------------
    // Sorting
    // -------------------------------------------------------------------

    pub fn apply_sort(&mut self) {
        let col = self.sort_column.as_str();
        let asc = self.sort_ascending;

        self.entries.sort_by(|a, b| {
            let dir_ord = b.is_dir.cmp(&a.is_dir);
            if dir_ord != std::cmp::Ordering::Equal {
                return dir_ord;
            }
            let ord = match col {
                COL_NAME => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                COL_EXT => {
                    let ext_a = a.name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
                    let ext_b = b.name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
                    ext_a.cmp(&ext_b)
                }
                COL_SIZE => a.size.cmp(&b.size),
                COL_MODIFIED => a.modified.cmp(&b.modified),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { ord } else { ord.reverse() }
        });
    }

    /// Get current column visibility and display settings.
    pub fn column_visibility(&self) -> ColumnVisibility {
        ColumnVisibility {
            ext: self.col_ext_visible,
            size: self.col_size_visible,
            modified: self.col_modified_visible,
            show_hidden: self.show_hidden,
        }
    }

    /// Apply column visibility and display settings.
    pub fn set_column_visibility(&mut self, vis: &ColumnVisibility) {
        self.col_ext_visible = vis.ext;
        self.col_size_visible = vis.size;
        self.col_modified_visible = vis.modified;
        self.show_hidden = vis.show_hidden;
        self.dirty = true;
    }

    // -------------------------------------------------------------------
    // Mode switching
    // -------------------------------------------------------------------

    pub fn switch_to_local(&mut self) {
        if matches!(self.mode, PaneMode::Local) {
            return;
        }
        self.mode = PaneMode::Local;
        self.back_stack.clear();
        self.forward_stack.clear();
        self.home_path = self.local_home.clone();
        self.current_path = self.local_home.clone();
        self.path_input = self.local_home.clone();
        self.selected_row = None;
        self.load_entries(None);
    }

    pub fn switch_to_remote(&mut self, session_id: u64, host: &str, user: &str, api: &HostApi) {
        self.mode = PaneMode::Remote {
            session_id,
            host: host.to_string(),
            user: user.to_string(),
        };
        self.back_stack.clear();
        self.forward_stack.clear();

        // Resolve the actual home directory path via SFTP realpath(".").
        let home = remote::realpath(api, session_id, ".").unwrap_or_else(|_| ".".to_string());
        self.current_path = home.clone();
        self.path_input = home.clone();
        self.home_path = home;
        self.selected_row = None;
        self.load_entries(Some(api));
    }

    // -------------------------------------------------------------------
    // Event handling
    // -------------------------------------------------------------------

    /// Handle a widget event, stripping the pane prefix from IDs.
    /// Returns true if the event was handled.
    pub fn handle_widget_event(&mut self, event: &WidgetEvent, api: Option<&HostApi>) -> bool {
        match event {
            WidgetEvent::ButtonClick { id } => {
                let stripped = strip_prefix(id, &self.prefix);
                match stripped {
                    "back" => self.go_back(api),
                    "forward" => self.go_forward(api),
                    "home" => self.go_home(api),
                    "refresh" => self.refresh(api),
                    "toggle_hidden" => {
                        self.show_hidden = !self.show_hidden;
                        self.dirty = true;
                    }
                    _ => return false,
                }
                true
            }

            WidgetEvent::ToolbarInputSubmit { id, value } => {
                if strip_prefix(id, &self.prefix) == "path" {
                    self.navigate_to(value, api);
                    true
                } else {
                    false
                }
            }

            WidgetEvent::TextInputSubmit { id, value } => {
                if strip_prefix(id, &self.prefix) == "path" {
                    self.navigate_to(value, api);
                    true
                } else {
                    false
                }
            }

            WidgetEvent::TableActivate { id, row_id, .. } => {
                if !id.starts_with(&self.prefix) {
                    return false;
                }
                if let Some(entry) = self.entries.iter().find(|e| e.name == *row_id) {
                    if entry.is_dir {
                        let new_path = if self.current_path.ends_with('/') || self.current_path == "." {
                            if self.current_path == "." {
                                entry.name.clone()
                            } else {
                                format!("{}{}", self.current_path, entry.name)
                            }
                        } else {
                            format!("{}/{}", self.current_path, entry.name)
                        };
                        self.navigate_to(&new_path, api);
                    }
                }
                true
            }

            WidgetEvent::TableSelect { id, row_id, .. } => {
                if !id.starts_with(&self.prefix) {
                    return false;
                }
                self.selected_row = Some(row_id.clone());
                self.dirty = true;
                true
            }

            WidgetEvent::TableSort { id, column, ascending } => {
                if !id.starts_with(&self.prefix) {
                    return false;
                }
                self.sort_column = column.clone();
                self.sort_ascending = *ascending;
                self.apply_sort();
                self.dirty = true;
                true
            }

            WidgetEvent::TableHeaderContextMenu { id, column } => {
                if !id.starts_with(&self.prefix) {
                    return false;
                }
                match column.as_str() {
                    COL_EXT => self.col_ext_visible = !self.col_ext_visible,
                    COL_SIZE => self.col_size_visible = !self.col_size_visible,
                    COL_MODIFIED => self.col_modified_visible = !self.col_modified_visible,
                    _ => {}
                }
                self.dirty = true;
                true
            }

            WidgetEvent::TableContextMenu { id, row_id, action } => {
                if !id.starts_with(&self.prefix) {
                    return false;
                }
                self.handle_context_action(row_id, action, api);
                true
            }

            _ => false,
        }
    }

    fn handle_context_action(&mut self, row_id: &str, action: &str, api: Option<&HostApi>) {
        match action {
            "new_folder" => {
                if let Some(api) = api {
                    let msg = std::ffi::CString::new("Enter folder name:").unwrap();
                    let default = std::ffi::CString::new("New Folder").unwrap();
                    let result_ptr = (api.show_prompt)(msg.as_ptr(), default.as_ptr());
                    if !result_ptr.is_null() {
                        let name = unsafe { std::ffi::CStr::from_ptr(result_ptr) }
                            .to_string_lossy()
                            .to_string();
                        (api.free_string)(result_ptr);
                        if !name.is_empty() {
                            let new_path = format!("{}/{}", self.current_path, name);
                            let result = match &self.mode {
                                PaneMode::Local => {
                                    std::fs::create_dir(&new_path).map_err(|e| e.to_string())
                                }
                                PaneMode::Remote { session_id, .. } => {
                                    remote::mkdir(api, *session_id, &new_path)
                                }
                            };
                            if let Err(e) = result {
                                self.error = Some(format!("Failed to create folder: {e}"));
                            }
                            self.refresh(Some(api));
                        }
                    }
                }
            }

            "delete" => {
                if let (Some(api), Some(entry)) = (api, self.entries.iter().find(|e| e.name == row_id)) {
                    let msg = std::ffi::CString::new(format!("Delete \"{}\"?", entry.name)).unwrap();
                    if (api.show_confirm)(msg.as_ptr()) {
                        let path = format!("{}/{}", self.current_path, entry.name);
                        let is_dir = entry.is_dir;
                        let result = match &self.mode {
                            PaneMode::Local => {
                                if is_dir {
                                    std::fs::remove_dir_all(&path).map_err(|e| e.to_string())
                                } else {
                                    std::fs::remove_file(&path).map_err(|e| e.to_string())
                                }
                            }
                            PaneMode::Remote { session_id, .. } => {
                                remote::delete(api, *session_id, &path, is_dir)
                            }
                        };
                        if let Err(e) = result {
                            self.error = Some(format!("Delete failed: {e}"));
                        }
                        self.refresh(Some(api));
                    }
                }
            }

            "rename" => {
                if let (Some(api), Some(entry)) = (api, self.entries.iter().find(|e| e.name == row_id)) {
                    let msg = std::ffi::CString::new("Enter new name:").unwrap();
                    let default = std::ffi::CString::new(entry.name.as_str()).unwrap();
                    let result_ptr = (api.show_prompt)(msg.as_ptr(), default.as_ptr());
                    if !result_ptr.is_null() {
                        let new_name = unsafe { std::ffi::CStr::from_ptr(result_ptr) }
                            .to_string_lossy()
                            .to_string();
                        (api.free_string)(result_ptr);
                        if !new_name.is_empty() && new_name != entry.name {
                            let from = format!("{}/{}", self.current_path, entry.name);
                            let to = format!("{}/{}", self.current_path, new_name);
                            let result = match &self.mode {
                                PaneMode::Local => {
                                    std::fs::rename(&from, &to).map_err(|e| e.to_string())
                                }
                                PaneMode::Remote { session_id, .. } => {
                                    remote::rename(api, *session_id, &from, &to)
                                }
                            };
                            if let Err(e) = result {
                                self.error = Some(format!("Rename failed: {e}"));
                            }
                            self.refresh(Some(api));
                        }
                    }
                }
            }

            "copy_path" => {
                if let Some(api) = api {
                    let path = format!("{}/{}", self.current_path, row_id);
                    let c = std::ffi::CString::new(path).unwrap();
                    (api.clipboard_set)(c.as_ptr());
                }
            }

            _ => {}
        }
    }

    // -------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------

    /// Title label for this pane.
    pub fn title(&self) -> String {
        match &self.mode {
            PaneMode::Local => {
                hostname::get()
                    .ok()
                    .and_then(|h| h.into_string().ok())
                    .unwrap_or_else(|| "localhost".to_string())
            }
            PaneMode::Remote { host, user, .. } => format!("{user}@{host}"),
        }
    }

    /// Full path to the selected file, if any.
    pub fn selected_path(&self) -> Option<String> {
        self.selected_row.as_ref().map(|name| {
            if self.current_path.ends_with('/') {
                format!("{}{}", self.current_path, name)
            } else {
                format!("{}/{}", self.current_path, name)
            }
        })
    }

    /// Whether the selected entry is a directory.
    pub fn selected_is_dir(&self) -> bool {
        self.selected_row.as_ref().map_or(false, |name| {
            self.entries.iter().any(|e| e.name == *name && e.is_dir)
        })
    }

    /// Size of the selected entry in bytes (0 if not found or directory).
    pub fn selected_size(&self) -> u64 {
        self.selected_row.as_ref().map_or(0, |name| {
            self.entries.iter().find(|e| e.name == *name).map_or(0, |e| e.size)
        })
    }

    /// Render this pane as a list of widgets (toolbar + table + footer).
    pub fn render_widgets(&self) -> Vec<Widget> {
        let p = &self.prefix;
        let mut widgets = Vec::new();

        // Label showing the pane title (hostname or user@host).
        widgets.push(Widget::Label {
            text: self.title(),
            style: Some(TextStyle::Secondary),
        });

        // Toolbar.
        widgets.push(Widget::Toolbar {
            id: Some(format!("{p}_nav")),
            items: vec![
                ToolbarItem::Button {
                    id: format!("{p}_back"),
                    icon: Some("go-previous".into()),
                    label: None,
                    tooltip: Some("Back".into()),
                    enabled: Some(!self.back_stack.is_empty()),
                },
                ToolbarItem::Button {
                    id: format!("{p}_forward"),
                    icon: Some("go-next".into()),
                    label: None,
                    tooltip: Some("Forward".into()),
                    enabled: Some(!self.forward_stack.is_empty()),
                },
                ToolbarItem::TextInput {
                    id: format!("{p}_path"),
                    value: self.path_input.clone(),
                    hint: Some("Path...".into()),
                },
                ToolbarItem::Button {
                    id: format!("{p}_home"),
                    icon: Some("go-home".into()),
                    label: None,
                    tooltip: Some("Home".into()),
                    enabled: None,
                },
                ToolbarItem::Button {
                    id: format!("{p}_refresh"),
                    icon: Some("refresh".into()),
                    label: None,
                    tooltip: Some("Refresh".into()),
                    enabled: None,
                },
                ToolbarItem::Button {
                    id: format!("{p}_toggle_hidden"),
                    icon: None,
                    label: Some(if self.show_hidden { ".*" } else { ".*" }.into()),
                    tooltip: Some(if self.show_hidden {
                        "Hide hidden files".into()
                    } else {
                        "Show hidden files".into()
                    }),
                    enabled: None,
                },
            ],
        });

        // Error.
        if let Some(err) = &self.error {
            widgets.push(Widget::Label {
                text: err.clone(),
                style: Some(TextStyle::Error),
            });
        }

        // Table.
        let mut columns = vec![TableColumn {
            id: COL_NAME.into(),
            label: "Name".into(),
            sortable: Some(true),
            width: None,
            visible: None,
        }];
        columns.push(TableColumn {
            id: COL_EXT.into(),
            label: "Ext".into(),
            sortable: Some(true),
            width: Some(50.0),
            visible: Some(self.col_ext_visible),
        });
        columns.push(TableColumn {
            id: COL_SIZE.into(),
            label: "Size".into(),
            sortable: Some(true),
            width: Some(55.0),
            visible: Some(self.col_size_visible),
        });
        columns.push(TableColumn {
            id: COL_MODIFIED.into(),
            label: "Modified".into(),
            sortable: Some(true),
            width: Some(100.0),
            visible: Some(self.col_modified_visible),
        });

        let rows: Vec<TableRow> = self
            .entries
            .iter()
            .filter(|entry| self.show_hidden || !entry.name.starts_with('.'))
            .map(|entry| {
                let icon = if entry.is_dir { "folder" } else { "file" };
                let ext_label = format::extension_label(&entry.name, entry.is_dir);
                let size_label = if entry.is_dir {
                    "<DIR>".to_string()
                } else {
                    format::format_size(entry.size)
                };
                let date_label = entry
                    .modified
                    .map(format::format_date)
                    .unwrap_or_else(|| "\u{2014}".to_string());

                TableRow {
                    id: entry.name.clone(),
                    cells: vec![
                        TableCell::Rich {
                            text: entry.name.clone(),
                            icon: Some(icon.into()),
                            badge: None,
                        },
                        TableCell::Text(ext_label),
                        TableCell::Text(size_label),
                        TableCell::Text(date_label),
                    ],
                    context_menu: Some(vec![
                        ContextMenuItem {
                            id: "new_folder".into(),
                            label: "New Folder".into(),
                            icon: Some("folder-new".into()),
                            enabled: None,
                            shortcut: None,
                        },
                        ContextMenuItem {
                            id: "rename".into(),
                            label: "Rename".into(),
                            icon: None,
                            enabled: None,
                            shortcut: None,
                        },
                        ContextMenuItem {
                            id: "delete".into(),
                            label: "Delete".into(),
                            icon: None,
                            enabled: None,
                            shortcut: None,
                        },
                        ContextMenuItem {
                            id: "copy_path".into(),
                            label: "Copy Path".into(),
                            icon: None,
                            enabled: None,
                            shortcut: None,
                        },
                    ]),
                }
            })
            .collect();

        widgets.push(Widget::Table {
            id: format!("{p}_files"),
            columns,
            rows,
            sort_column: Some(self.sort_column.clone()),
            sort_ascending: Some(self.sort_ascending),
            selected_row: self.selected_row.clone(),
        });

        // Footer.
        let visible_count = if self.show_hidden {
            self.entries.len()
        } else {
            self.entries.iter().filter(|e| !e.name.starts_with('.')).count()
        };
        let total_count = self.entries.len();
        let footer = if visible_count == total_count {
            format!("{visible_count} items")
        } else {
            format!("{visible_count} items ({} hidden)", total_count - visible_count)
        };
        widgets.push(Widget::Label {
            text: footer,
            style: Some(TextStyle::Secondary),
        });

        widgets
    }
}

/// Strip a pane prefix from an ID (e.g. "local_back" → "back").
fn strip_prefix<'a>(id: &'a str, prefix: &str) -> &'a str {
    id.strip_prefix(prefix)
        .and_then(|s| s.strip_prefix('_'))
        .unwrap_or(id)
}
