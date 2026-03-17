//! Stub JVM plugin module — used when the Java SDK JAR was not found at build time.

pub mod runtime {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::bus::PluginBus;
    use crate::native::{LoadError, PluginMeta};

    pub struct JavaPluginManager {
        _private: (),
    }

    unsafe impl Send for JavaPluginManager {}

    impl JavaPluginManager {
        pub fn new(_bus: Arc<PluginBus>, _host_api: conch_plugin_sdk::HostApi) -> Self {
            log::warn!(
                "JVM plugin support unavailable — binary was built without the Java SDK JAR. \
                 Build it with: make -C java-sdk build"
            );
            Self { _private: () }
        }

        pub fn discover(&mut self, _dir: &Path) -> Vec<(PathBuf, PluginMeta)> {
            Vec::new()
        }

        pub fn load_plugin(&mut self, _jar_path: &Path) -> Result<PluginMeta, LoadError> {
            Err(LoadError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "JVM plugin support was not compiled in (Java SDK JAR missing at build time)",
            )))
        }

        pub fn unload_plugin(&mut self, name: &str) -> Result<(), LoadError> {
            Err(LoadError::NotLoaded(name.to_string()))
        }

        pub fn loaded_plugins(&self) -> Vec<&PluginMeta> {
            Vec::new()
        }

        pub fn is_loaded(&self, _name: &str) -> bool {
            false
        }

        pub fn loaded_count(&self) -> usize {
            0
        }

        pub fn shutdown_all(&mut self) {}
    }
}
