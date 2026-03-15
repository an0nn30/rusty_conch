//! Plugin discovery, auto-loading, persistence, and render polling.

use std::collections::HashSet;
use std::path::PathBuf;

use conch_core::config;
use conch_plugin::bus::PluginMail;
use conch_plugin::lua::runner::{DiscoveredLuaPlugin, RunningLuaPlugin};
use conch_plugin_sdk::PanelLocation;
use tokio::sync::oneshot;

use crate::app::ConchApp;
use crate::host::bridge;
use crate::host::plugin_manager_ui::{PluginEntry, PluginSource};

impl ConchApp {
    /// Scan search paths for native and Lua plugins, updating the plugin manager.
    pub(crate) fn discover_plugins(&mut self) {
        let mut entries = Vec::new();
        let configured = &self.state.user_config.conch.plugins.search_paths;

        // Build search directories. Default platform paths are always included;
        // user-configured paths are appended so they can override or supplement.
        let mut dirs = Vec::new();

        // Development paths (only useful when running from the repo).
        dirs.push(PathBuf::from("target/debug"));
        dirs.push(PathBuf::from("target/release"));
        dirs.push(PathBuf::from("examples/plugins"));

        // Exe directory and sibling paths (handles installed builds).
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                // Same directory as the binary (Windows, portable builds).
                dirs.push(exe_dir.to_path_buf());
                // plugins/ subdirectory next to the binary.
                dirs.push(exe_dir.join("plugins"));
                // macOS app bundle: Conch.app/Contents/Plugins/
                if let Some(contents_dir) = exe_dir.parent() {
                    dirs.push(contents_dir.join("Plugins"));
                }
                // Linux: /opt/conch/lib/ when binary is in /opt/conch/bin/
                if let Some(install_root) = exe_dir.parent() {
                    dirs.push(install_root.join("lib"));
                }
            }
        }

        // Standard Linux install path.
        #[cfg(target_os = "linux")]
        {
            dirs.push(PathBuf::from("/opt/conch/lib"));
            dirs.push(PathBuf::from("/usr/lib/conch/plugins"));
        }

        // User plugin directory (~/.config/conch/plugins/ or platform equivalent).
        if let Some(config_dir) = dirs::config_dir() {
            dirs.push(config_dir.join("conch").join("plugins"));
        }

        // Append user-configured search paths (these supplement the defaults).
        for p in configured {
            let expanded = if p.starts_with("~/") {
                dirs::home_dir()
                    .map(|home| home.join(&p[2..]))
                    .unwrap_or_else(|| PathBuf::from(p))
            } else {
                PathBuf::from(p)
            };
            dirs.push(expanded);
        }

        let search_dirs = dirs;

        for dir in &search_dirs {
            if !dir.is_dir() {
                continue;
            }

            // Discover native plugins (.dylib/.so/.dll).
            if let Ok(paths) = conch_plugin::native::library::discover_library_paths(dir) {
                for path in paths {
                    match unsafe { conch_plugin::native::PluginLibrary::load(&path) } {
                        Ok(lib) => {
                            let meta = unsafe { lib.read_info() };
                            entries.push(PluginEntry {
                                name: meta.name,
                                description: meta.description,
                                version: meta.version,
                                plugin_type: meta.plugin_type,
                                panel_location: meta.panel_location,
                                source: PluginSource::Native,
                                path,
                            });
                        }
                        Err(_) => {} // Not a valid Conch plugin.
                    }
                }
            }

            // Discover Java plugins (.jar).
            for (path, meta) in self.java_plugin_mgr.discover(dir) {
                entries.push(PluginEntry {
                    name: meta.name,
                    description: meta.description,
                    version: meta.version,
                    plugin_type: meta.plugin_type,
                    panel_location: meta.panel_location,
                    source: PluginSource::Java,
                    path,
                });
            }

            // Discover Lua plugins (.lua).
            for plugin in conch_plugin::lua::runner::discover(dir) {
                entries.push(PluginEntry {
                    name: plugin.meta.name,
                    description: plugin.meta.description,
                    version: plugin.meta.version,
                    plugin_type: plugin.meta.plugin_type,
                    panel_location: plugin.meta.panel_location,
                    source: PluginSource::Lua,
                    path: plugin.path,
                });
            }
        }

        // Deduplicate by name (keep first occurrence).
        let mut seen = HashSet::new();
        entries.retain(|e| seen.insert(e.name.clone()));

        log::info!("Discovered {} plugins", entries.len());
        for e in &entries {
            log::info!("  - {} v{} ({}) [{}]", e.name, e.version, e.source, e.path.display());
        }

        self.plugin_manager.set_plugins(entries);
    }

    /// Load plugins that were enabled in the previous session.
    pub(crate) fn auto_load_plugins(&mut self) {
        let to_load: Vec<String> = self.state.persistent.loaded_plugins.clone();
        for name in &to_load {
            if let Some(entry) = self.plugin_manager.find_plugin(name) {
                let source = entry.source;
                let path = entry.path.clone();
                match source {
                    PluginSource::Native => {
                        match self.native_plugin_mgr.load_plugin(&path) {
                            Ok(meta) => {
                                log::info!("Auto-loaded native plugin '{}' v{}", meta.name, meta.version);
                                self.plugin_manager.set_loaded(name, true);
                            }
                            Err(e) => {
                                log::warn!("Failed to auto-load plugin '{name}': {e}");
                            }
                        }
                    }
                    PluginSource::Java => {
                        match self.java_plugin_mgr.load_plugin(&path) {
                            Ok(meta) => {
                                log::info!("Auto-loaded Java plugin '{}' v{}", meta.name, meta.version);
                                self.plugin_manager.set_loaded(name, true);
                            }
                            Err(e) => {
                                log::warn!("Failed to auto-load Java plugin '{name}': {e}");
                            }
                        }
                    }
                    PluginSource::Lua => {
                        match self.load_lua_plugin(name, &path) {
                            Ok(()) => {
                                log::info!("Auto-loaded Lua plugin '{name}'");
                                self.plugin_manager.set_loaded(name, true);
                            }
                            Err(e) => {
                                log::warn!("Failed to auto-load Lua plugin '{name}': {e}");
                            }
                        }
                    }
                }
            } else {
                log::warn!("Previously loaded plugin '{name}' not found during discovery");
            }
        }
    }

    /// Persist the current set of loaded plugin names to state.toml.
    pub(crate) fn save_loaded_plugins(&mut self) {
        let mut loaded: Vec<String> = self
            .native_plugin_mgr
            .loaded_plugins()
            .iter()
            .map(|m| m.name.clone())
            .collect();
        // Include Java plugins.
        loaded.extend(
            self.java_plugin_mgr
                .loaded_plugins()
                .iter()
                .map(|m| m.name.clone()),
        );
        // Include Lua plugins.
        for name in self.lua_plugins.keys() {
            loaded.push(name.clone());
        }
        self.state.persistent.loaded_plugins = loaded;
        let _ = config::save_persistent_state(&self.state.persistent);
    }

    /// Handle a single plugin manager action (load/unload/refresh).
    pub(crate) fn handle_plugin_manager_action(
        &mut self,
        action: crate::host::plugin_manager_ui::PluginManagerAction,
    ) {
        use crate::host::plugin_manager_ui::PluginManagerAction;
        match action {
            PluginManagerAction::Refresh => {
                self.discover_plugins();
            }
            PluginManagerAction::Load(name) => {
                if let Some(entry) = self.plugin_manager.find_plugin(&name) {
                    let source = entry.source;
                    let path = entry.path.clone();
                    match source {
                        PluginSource::Native => {
                            match self.native_plugin_mgr.load_plugin(&path) {
                                Ok(meta) => {
                                    log::info!("Loaded plugin '{}' v{}", meta.name, meta.version);
                                    self.plugin_manager.set_loaded(&name, true);
                                    self.save_loaded_plugins();
                                }
                                Err(e) => {
                                    log::error!("Failed to load plugin '{name}': {e}");
                                }
                            }
                        }
                        PluginSource::Java => {
                            match self.java_plugin_mgr.load_plugin(&path) {
                                Ok(meta) => {
                                    log::info!("Loaded Java plugin '{}' v{}", meta.name, meta.version);
                                    self.plugin_manager.set_loaded(&name, true);
                                    self.save_loaded_plugins();
                                }
                                Err(e) => {
                                    log::error!("Failed to load Java plugin '{name}': {e}");
                                }
                            }
                        }
                        PluginSource::Lua => {
                            match self.load_lua_plugin(&name, &path) {
                                Ok(()) => {
                                    log::info!("Loaded Lua plugin '{name}'");
                                    self.plugin_manager.set_loaded(&name, true);
                                    self.save_loaded_plugins();
                                }
                                Err(e) => {
                                    log::error!("Failed to load Lua plugin '{name}': {e}");
                                }
                            }
                        }
                    }
                }
            }
            PluginManagerAction::Unload(name) => {
                // Try Lua first, then Java, then native.
                if self.lua_plugins.contains_key(&name) {
                    self.unload_lua_plugin(&name);
                    self.plugin_manager.set_loaded(&name, false);
                    self.save_loaded_plugins();
                } else {
                    let result = if self.java_plugin_mgr.is_loaded(&name) {
                        self.java_plugin_mgr.unload_plugin(&name)
                    } else {
                        self.native_plugin_mgr.unload_plugin(&name)
                    };
                    match result {
                        Ok(()) => {
                            log::info!("Unloaded plugin '{name}'");
                            self.panel_registry.lock().remove_by_plugin(&name);
                            self.render_pending.remove(&name);
                            self.render_cache.remove(&name);
                            self.plugin_manager.set_loaded(&name, false);
                            self.save_loaded_plugins();
                        }
                        Err(e) => {
                            log::error!("Failed to unload plugin '{name}': {e}");
                        }
                    }
                }
            }
        }
    }

    /// Load a Lua plugin by reading its source and spawning a runner thread.
    fn load_lua_plugin(&mut self, name: &str, path: &PathBuf) -> Result<(), String> {
        if self.lua_plugins.contains_key(name) {
            return Err(format!("Lua plugin '{name}' is already loaded"));
        }

        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        let meta = conch_plugin::lua::metadata::parse_lua_metadata(&source);
        let discovered = DiscoveredLuaPlugin {
            path: path.clone(),
            source,
            meta,
        };

        // Register on the bus and get the mailbox.
        let mailbox_rx = self.plugin_bus.register_plugin(name);
        let mailbox_tx = self.plugin_bus.sender_for(name).unwrap();
        let host_api = self.native_plugin_mgr.host_api_ptr();

        let running = conch_plugin::lua::runner::spawn_lua_plugin(
            &discovered,
            host_api,
            mailbox_tx,
            mailbox_rx,
        )?;

        self.lua_plugins.insert(name.to_string(), running);
        Ok(())
    }

    /// Unload a Lua plugin: send shutdown, join thread, clean up.
    fn unload_lua_plugin(&mut self, name: &str) {
        if let Some(mut running) = self.lua_plugins.remove(name) {
            // Send shutdown signal.
            if running.sender.try_send(PluginMail::Shutdown).is_err() {
                log::warn!("Lua plugin [{name}]: failed to send shutdown");
            }
            // Wait for thread to exit.
            if let Some(handle) = running.thread.take() {
                let _ = handle.join();
            }
            // Clean up bus, panels, caches.
            self.plugin_bus.unregister_plugin(name);
            self.panel_registry.lock().remove_by_plugin(name);
            self.render_pending.remove(name);
            self.render_cache.remove(name);
            log::info!("Unloaded Lua plugin '{name}'");
        }
    }

    /// Minimum interval between render requests to the same plugin.
    /// Caps plugin render polling to ~4fps when idle, which is plenty for
    /// panel UIs. User interactions (widget events) trigger immediate
    /// re-renders via the event path.
    const RENDER_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

    /// Poll pending render requests and fire new ones for panel plugins.
    pub(crate) fn poll_plugin_renders(&mut self) {
        // Check pending render responses.
        let pending_names: Vec<String> = self.render_pending.keys().cloned().collect();
        for name in pending_names {
            let ready = {
                let rx = self.render_pending.get_mut(&name).unwrap();
                match rx.try_recv() {
                    Ok(json) => Some(json),
                    Err(oneshot::error::TryRecvError::Empty) => None,
                    Err(oneshot::error::TryRecvError::Closed) => {
                        // Channel closed — remove the pending entry.
                        Some("[]".to_string())
                    }
                }
            };
            if let Some(json) = ready {
                self.render_cache.insert(name.clone(), json);
                self.render_pending.remove(&name);
            }
        }

        // Fire new render requests for loaded panel plugins, throttled to
        // avoid driving the app at 60fps when plugins have nothing new.
        let now = std::time::Instant::now();
        let panels: Vec<(String, String)> = {
            let reg = self.panel_registry.lock();
            reg.panels()
                .map(|(_, info)| (info.plugin_name.clone(), info.name.clone()))
                .collect()
        };
        for (plugin_name, _panel_name) in panels {
            if self.render_pending.contains_key(&plugin_name) {
                continue; // Already waiting for a response.
            }
            // Throttle: skip if we sent a request recently.
            if let Some(last) = self.render_last_request.get(&plugin_name) {
                if now.duration_since(*last) < Self::RENDER_POLL_INTERVAL {
                    continue;
                }
            }
            if let Some(sender) = self.plugin_bus.sender_for(&plugin_name) {
                let (tx, rx) = oneshot::channel();
                if sender.try_send(PluginMail::RenderRequest { reply: tx }).is_ok() {
                    self.render_pending.insert(plugin_name.clone(), rx);
                    self.render_last_request.insert(plugin_name, now);
                }
            }
        }
    }
}
