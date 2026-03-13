//! Compile-time embedded PNG icons with a texture cache for egui.
//!
//! Icon names are shared with the plugin SDK — plugins reference icons by
//! string name (e.g., `"folder"`, `"server"`) in their widget trees.

use std::collections::HashMap;

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

// ---------------------------------------------------------------------------
// Embedded PNGs
// ---------------------------------------------------------------------------

const FILE_DARK_PNG: &[u8] = include_bytes!("../icons/file-dark.png");
const FILE_LIGHT_PNG: &[u8] = include_bytes!("../icons/file-light.png");
const FOLDER_PNG: &[u8] = include_bytes!("../icons/folder.png");
const FOLDER_OPEN_PNG: &[u8] = include_bytes!("../icons/folder-open.png");
const SERVER_PNG: &[u8] = include_bytes!("../icons/server.png");
const NETWORK_SERVER_PNG: &[u8] = include_bytes!("../icons/network-server.png");
const TERMINAL_PNG: &[u8] = include_bytes!("../icons/terminal.png");
const TAB_SESSIONS_DARK_PNG: &[u8] = include_bytes!("../icons/tab-sessions-dark.png");
const TAB_SESSIONS_LIGHT_PNG: &[u8] = include_bytes!("../icons/tab-sessions-light.png");
const TAB_FILES_PNG: &[u8] = include_bytes!("../icons/tab-files.png");
const TAB_TOOLS_PNG: &[u8] = include_bytes!("../icons/tab-tools.png");
const TAB_MACROS_PNG: &[u8] = include_bytes!("../icons/tab-macros.png");
const GO_DOWN_PNG: &[u8] = include_bytes!("../icons/go-down.png");
const GO_UP_DARK_PNG: &[u8] = include_bytes!("../icons/go-up-dark.png");
const GO_UP_LIGHT_PNG: &[u8] = include_bytes!("../icons/go-up-light.png");
const GO_HOME_DARK_PNG: &[u8] = include_bytes!("../icons/go-home-dark.png");
const GO_HOME_LIGHT_PNG: &[u8] = include_bytes!("../icons/go-home-light.png");
const REFRESH_DARK_PNG: &[u8] = include_bytes!("../icons/view-refresh-dark.png");
const REFRESH_LIGHT_PNG: &[u8] = include_bytes!("../icons/view-refresh-light.png");
const FOLDER_NEW_DARK_PNG: &[u8] = include_bytes!("../icons/folder-new-dark.png");
const FOLDER_NEW_LIGHT_PNG: &[u8] = include_bytes!("../icons/folder-new-light.png");
const SIDEBAR_FOLDER_DARK_PNG: &[u8] = include_bytes!("../icons/sidebar-folder-dark.png");
const SIDEBAR_FOLDER_LIGHT_PNG: &[u8] = include_bytes!("../icons/sidebar-folder-light.png");
const GO_PREVIOUS_DARK_PNG: &[u8] = include_bytes!("../icons/go-previous-dark.png");
const GO_PREVIOUS_LIGHT_PNG: &[u8] = include_bytes!("../icons/go-previous-light.png");
const GO_NEXT_DARK_PNG: &[u8] = include_bytes!("../icons/go-next-dark.png");
const GO_NEXT_LIGHT_PNG: &[u8] = include_bytes!("../icons/go-next-light.png");
const COMPUTER_DARK_PNG: &[u8] = include_bytes!("../icons/computer-dark.png");
const COMPUTER_LIGHT_PNG: &[u8] = include_bytes!("../icons/computer-light.png");
const TAB_CLOSE_DARK_PNG: &[u8] = include_bytes!("../icons/tab-close-dark.png");
const TAB_CLOSE_LIGHT_PNG: &[u8] = include_bytes!("../icons/tab-close-light.png");
const TRANSFER_DOWN_DARK_PNG: &[u8] = include_bytes!("../icons/transfer-down-dark.png");
const TRANSFER_DOWN_LIGHT_PNG: &[u8] = include_bytes!("../icons/transfer-down-light.png");
const TRANSFER_UP_DARK_PNG: &[u8] = include_bytes!("../icons/transfer-up-dark.png");
const TRANSFER_UP_LIGHT_PNG: &[u8] = include_bytes!("../icons/transfer-up-light.png");

// ---------------------------------------------------------------------------
// Icon enum
// ---------------------------------------------------------------------------

/// Semantic icon identifiers. Plugins reference these by string name via
/// the `icon` field in widget trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    File,
    Folder,
    FolderOpen,
    Server,
    NetworkServer,
    Terminal,
    TabSessions,
    TabFiles,
    TabTools,
    TabMacros,
    GoDown,
    GoUp,
    GoHome,
    Refresh,
    FolderNew,
    SidebarFolder,
    GoPrevious,
    GoNext,
    Computer,
    TabClose,
    TransferDown,
    TransferUp,
}

/// Resolve a string icon name (as used in plugin widget JSON) to an `Icon`.
///
/// Accepts kebab-case, snake_case, and some common aliases.
pub fn icon_from_name(name: &str) -> Option<Icon> {
    match name {
        "file" => Some(Icon::File),
        "folder" => Some(Icon::Folder),
        "folder-open" | "folder_open" => Some(Icon::FolderOpen),
        "server" => Some(Icon::Server),
        "network-server" | "network_server" => Some(Icon::NetworkServer),
        "terminal" => Some(Icon::Terminal),
        "tab-sessions" | "tab_sessions" | "sessions" => Some(Icon::TabSessions),
        "tab-files" | "tab_files" => Some(Icon::TabFiles),
        "tab-tools" | "tab_tools" => Some(Icon::TabTools),
        "tab-macros" | "tab_macros" => Some(Icon::TabMacros),
        "go-down" | "go_down" => Some(Icon::GoDown),
        "go-up" | "go_up" => Some(Icon::GoUp),
        "go-home" | "go_home" | "home" => Some(Icon::GoHome),
        "refresh" | "view-refresh" | "view_refresh" => Some(Icon::Refresh),
        "folder-new" | "folder_new" | "new-folder" => Some(Icon::FolderNew),
        "sidebar-folder" | "sidebar_folder" => Some(Icon::SidebarFolder),
        "go-previous" | "go_previous" | "previous" | "back" => Some(Icon::GoPrevious),
        "go-next" | "go_next" | "next" | "forward" => Some(Icon::GoNext),
        "computer" | "monitor" => Some(Icon::Computer),
        "tab-close" | "tab_close" | "close" => Some(Icon::TabClose),
        "transfer-down" | "transfer_down" | "download" => Some(Icon::TransferDown),
        "transfer-up" | "transfer_up" | "upload" => Some(Icon::TransferUp),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Theme variant helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TexKey {
    Single(Icon),
    Dark(Icon),
    Light(Icon),
}

const THEMED_ICONS: &[Icon] = &[
    Icon::File, Icon::TabSessions, Icon::TabClose, Icon::GoUp, Icon::GoHome,
    Icon::Refresh, Icon::FolderNew, Icon::SidebarFolder, Icon::GoPrevious,
    Icon::GoNext, Icon::Computer, Icon::TransferDown, Icon::TransferUp,
];

fn is_themed(icon: Icon) -> bool {
    THEMED_ICONS.contains(&icon)
}

fn single_bytes(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::File => FILE_DARK_PNG,
        Icon::Computer => COMPUTER_DARK_PNG,
        Icon::Folder => FOLDER_PNG,
        Icon::FolderOpen => FOLDER_OPEN_PNG,
        Icon::Server => SERVER_PNG,
        Icon::NetworkServer => NETWORK_SERVER_PNG,
        Icon::Terminal => TERMINAL_PNG,
        Icon::TabSessions => TAB_SESSIONS_DARK_PNG,
        Icon::TabClose => TAB_CLOSE_DARK_PNG,
        Icon::TabFiles => TAB_FILES_PNG,
        Icon::TabTools => TAB_TOOLS_PNG,
        Icon::TabMacros => TAB_MACROS_PNG,
        Icon::GoDown => GO_DOWN_PNG,
        Icon::GoUp => GO_UP_DARK_PNG,
        Icon::GoHome => GO_HOME_DARK_PNG,
        Icon::Refresh => REFRESH_DARK_PNG,
        Icon::FolderNew => FOLDER_NEW_DARK_PNG,
        Icon::SidebarFolder => SIDEBAR_FOLDER_DARK_PNG,
        Icon::GoPrevious => GO_PREVIOUS_DARK_PNG,
        Icon::GoNext => GO_NEXT_DARK_PNG,
        Icon::TransferDown => TRANSFER_DOWN_DARK_PNG,
        Icon::TransferUp => TRANSFER_UP_DARK_PNG,
    }
}

fn dark_bytes(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::File => FILE_DARK_PNG,
        Icon::Computer => COMPUTER_DARK_PNG,
        Icon::TabSessions => TAB_SESSIONS_DARK_PNG,
        Icon::TabClose => TAB_CLOSE_DARK_PNG,
        Icon::GoUp => GO_UP_DARK_PNG,
        Icon::GoHome => GO_HOME_DARK_PNG,
        Icon::Refresh => REFRESH_DARK_PNG,
        Icon::FolderNew => FOLDER_NEW_DARK_PNG,
        Icon::SidebarFolder => SIDEBAR_FOLDER_DARK_PNG,
        Icon::GoPrevious => GO_PREVIOUS_DARK_PNG,
        Icon::GoNext => GO_NEXT_DARK_PNG,
        Icon::TransferDown => TRANSFER_DOWN_DARK_PNG,
        Icon::TransferUp => TRANSFER_UP_DARK_PNG,
        _ => unreachable!(),
    }
}

fn light_bytes(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::File => FILE_LIGHT_PNG,
        Icon::Computer => COMPUTER_LIGHT_PNG,
        Icon::TabSessions => TAB_SESSIONS_LIGHT_PNG,
        Icon::TabClose => TAB_CLOSE_LIGHT_PNG,
        Icon::GoUp => GO_UP_LIGHT_PNG,
        Icon::GoHome => GO_HOME_LIGHT_PNG,
        Icon::Refresh => REFRESH_LIGHT_PNG,
        Icon::FolderNew => FOLDER_NEW_LIGHT_PNG,
        Icon::SidebarFolder => SIDEBAR_FOLDER_LIGHT_PNG,
        Icon::GoPrevious => GO_PREVIOUS_LIGHT_PNG,
        Icon::GoNext => GO_NEXT_LIGHT_PNG,
        Icon::TransferDown => TRANSFER_DOWN_LIGHT_PNG,
        Icon::TransferUp => TRANSFER_UP_LIGHT_PNG,
        _ => unreachable!(),
    }
}

fn tex_name(key: TexKey) -> String {
    match key {
        TexKey::Single(i) => format!("icon_{:?}", i),
        TexKey::Dark(i) => format!("icon_{:?}_dark", i),
        TexKey::Light(i) => format!("icon_{:?}_light", i),
    }
}

const ALL_SINGLE: &[Icon] = &[
    Icon::Folder,
    Icon::FolderOpen,
    Icon::Server,
    Icon::NetworkServer,
    Icon::Terminal,
    Icon::TabFiles,
    Icon::TabTools,
    Icon::TabMacros,
    Icon::GoDown,
];

// ---------------------------------------------------------------------------
// IconCache
// ---------------------------------------------------------------------------

/// Caches decoded PNG textures for use in egui widgets.
pub struct IconCache {
    textures: HashMap<TexKey, TextureHandle>,
}

impl IconCache {
    /// Decode all embedded PNGs and upload as egui textures.
    pub fn load(ctx: &Context) -> Self {
        let mut textures = HashMap::new();

        for &icon in ALL_SINGLE {
            let key = TexKey::Single(icon);
            if let Some(h) = decode_and_upload(ctx, &tex_name(key), single_bytes(icon)) {
                textures.insert(key, h);
            }
        }

        for &icon in THEMED_ICONS {
            let dk = TexKey::Dark(icon);
            if let Some(h) = decode_and_upload(ctx, &tex_name(dk), dark_bytes(icon)) {
                textures.insert(dk, h);
            }
            let lk = TexKey::Light(icon);
            if let Some(h) = decode_and_upload(ctx, &tex_name(lk), light_bytes(icon)) {
                textures.insert(lk, h);
            }
        }

        IconCache { textures }
    }

    /// Get a sized `Image` for the given icon (16x16).
    pub fn image(&self, icon: Icon) -> Option<egui::Image<'_>> {
        self.texture_handle(icon).map(|h| {
            egui::Image::new(egui::load::SizedTexture::new(h.id(), [16.0, 16.0]))
        })
    }

    /// Get a `TextureId` for painter-level drawing.
    pub fn texture_id(&self, icon: Icon) -> Option<egui::TextureId> {
        let key = if is_themed(icon) {
            TexKey::Light(icon)
        } else {
            TexKey::Single(icon)
        };
        self.textures.get(&key).map(|h| h.id())
    }

    /// Get the right `Image` for a themed icon given an explicit dark_mode flag.
    pub fn themed_image(&self, icon: Icon, dark_mode: bool) -> Option<egui::Image<'_>> {
        let key = if is_themed(icon) {
            if dark_mode { TexKey::Light(icon) } else { TexKey::Dark(icon) }
        } else {
            TexKey::Single(icon)
        };
        self.textures.get(&key).map(|h| {
            egui::Image::new(egui::load::SizedTexture::new(h.id(), [16.0, 16.0]))
        })
    }

    /// Look up an icon by string name (for plugin widget rendering).
    pub fn image_by_name(&self, name: &str, dark_mode: bool) -> Option<egui::Image<'_>> {
        icon_from_name(name).and_then(|icon| self.themed_image(icon, dark_mode))
    }

    fn texture_handle(&self, icon: Icon) -> Option<&TextureHandle> {
        if is_themed(icon) {
            self.textures.get(&TexKey::Light(icon))
        } else {
            self.textures.get(&TexKey::Single(icon))
        }
    }
}

fn decode_and_upload(ctx: &Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    let color_image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture(name, color_image, TextureOptions::LINEAR))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_from_name_kebab_case() {
        assert_eq!(icon_from_name("folder-open"), Some(Icon::FolderOpen));
        assert_eq!(icon_from_name("network-server"), Some(Icon::NetworkServer));
        assert_eq!(icon_from_name("go-home"), Some(Icon::GoHome));
    }

    #[test]
    fn icon_from_name_snake_case() {
        assert_eq!(icon_from_name("folder_open"), Some(Icon::FolderOpen));
        assert_eq!(icon_from_name("go_up"), Some(Icon::GoUp));
    }

    #[test]
    fn icon_from_name_aliases() {
        assert_eq!(icon_from_name("monitor"), Some(Icon::Computer));
        assert_eq!(icon_from_name("back"), Some(Icon::GoPrevious));
        assert_eq!(icon_from_name("forward"), Some(Icon::GoNext));
        assert_eq!(icon_from_name("download"), Some(Icon::TransferDown));
        assert_eq!(icon_from_name("upload"), Some(Icon::TransferUp));
        assert_eq!(icon_from_name("home"), Some(Icon::GoHome));
    }

    #[test]
    fn icon_from_name_unknown() {
        assert_eq!(icon_from_name("nonexistent"), None);
        assert_eq!(icon_from_name(""), None);
    }

    #[test]
    fn all_single_icons_have_bytes() {
        for &icon in ALL_SINGLE {
            assert!(!single_bytes(icon).is_empty());
        }
    }

    #[test]
    fn all_themed_icons_have_both_variants() {
        for &icon in THEMED_ICONS {
            assert!(!dark_bytes(icon).is_empty());
            assert!(!light_bytes(icon).is_empty());
        }
    }
}
