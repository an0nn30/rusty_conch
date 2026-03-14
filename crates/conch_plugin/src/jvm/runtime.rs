//! JVM plugin manager — discovery, loading, and lifecycle for `.jar` plugins.

use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::sys::jint;
use jni::{InitArgsBuilder, JNIEnv, JavaVM, NativeMethod};
use tokio::sync::mpsc;

use crate::bus::{PluginBus, PluginMail, QueryResponse};
use crate::native::lifecycle::LoadedPlugin;
use crate::native::{LoadError, PluginMeta};

/// Global pointer to the HostApi vtable. Set once by `JavaPluginManager::new()`.
/// JNI native methods read this to call back into the host.
static HOST_API_PTR: AtomicPtr<conch_plugin_sdk::HostApi> =
    AtomicPtr::new(std::ptr::null_mut());

/// Manages JVM plugin discovery, loading, and lifecycle.
pub struct JavaPluginManager {
    bus: Arc<PluginBus>,
    plugins: HashMap<String, LoadedPlugin>,
    jvm: Option<JavaVM>,
    sdk_jar_path: PathBuf,
    _host_api_box: Box<conch_plugin_sdk::HostApi>,
}

// SAFETY: JavaVM is Send+Sync. The HostApi pointer is stable (owned by _host_api_box).
unsafe impl Send for JavaPluginManager {}

impl JavaPluginManager {
    pub fn new(bus: Arc<PluginBus>, host_api: conch_plugin_sdk::HostApi, sdk_jar_path: PathBuf) -> Self {
        let mut boxed = Box::new(host_api);
        let ptr: *mut conch_plugin_sdk::HostApi = &mut *boxed;
        HOST_API_PTR.store(ptr, Ordering::Release);

        Self {
            bus,
            plugins: HashMap::new(),
            jvm: None,
            sdk_jar_path,
            _host_api_box: boxed,
        }
    }

    /// Lazily create the JVM with the SDK JAR in the classpath.
    fn ensure_jvm(&mut self) -> Result<&JavaVM, LoadError> {
        if self.jvm.is_some() {
            return Ok(self.jvm.as_ref().unwrap());
        }

        let classpath = format!("-Djava.class.path={}", self.sdk_jar_path.display());
        log::info!("jvm: starting JVM with classpath: {classpath}");

        let jvm_args = InitArgsBuilder::new()
            .version(jni::JNIVersion::V8)
            .option(&classpath)
            .build()
            .map_err(|e| LoadError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("JVM init args: {e}"),
            )))?;

        let jvm = JavaVM::new(jvm_args).map_err(|e| {
            LoadError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("JVM creation failed: {e}"),
            ))
        })?;

        // Register native methods for HostApi.
        {
            let mut env = jvm.attach_current_thread().map_err(|e| {
                LoadError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("JNI attach failed: {e}"),
                ))
            })?;
            register_host_natives(&mut env)?;
        }

        log::info!("jvm: JVM started successfully");
        self.jvm = Some(jvm);
        Ok(self.jvm.as_ref().unwrap())
    }

    /// Scan a directory for `.jar` files and probe their metadata.
    pub fn discover(&mut self, dir: &Path) -> Vec<(PathBuf, PluginMeta)> {
        let mut found = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return found,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jar") {
                continue;
            }
            match self.probe_jar_metadata(&path) {
                Ok(meta) => found.push((path, meta)),
                Err(e) => log::debug!("jvm: skipping {}: {e}", path.display()),
            }
        }
        found
    }

    /// Read plugin metadata from a JAR by loading it in the JVM.
    fn probe_jar_metadata(&mut self, jar_path: &Path) -> Result<PluginMeta, LoadError> {
        let class_name = read_plugin_class_from_jar(jar_path)?;

        let jvm = self.ensure_jvm()?;
        let mut env = jvm.attach_current_thread().map_err(|e| {
            LoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("JNI attach: {e}")))
        })?;

        // Load the JAR via URLClassLoader.
        let loader = create_url_classloader(&mut env, jar_path)?;
        let plugin_obj = instantiate_plugin(&mut env, &loader, &class_name)?;

        // Call getInfo().
        let info_obj = env
            .call_method(&plugin_obj, "getInfo", "()Lconch/plugin/PluginInfo;", &[])
            .map_err(|e| LoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("getInfo: {e}"))))?
            .l()
            .map_err(|e| LoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("getInfo obj: {e}"))))?;

        let meta = read_plugin_info(&mut env, &info_obj)?;
        Ok(meta)
    }

    /// Load and activate a Java plugin from a JAR.
    pub fn load_plugin(&mut self, jar_path: &Path) -> Result<PluginMeta, LoadError> {
        let class_name = read_plugin_class_from_jar(jar_path)?;
        let meta = self.probe_jar_metadata(jar_path)?;
        let name = meta.name.clone();

        if self.plugins.contains_key(&name) {
            return Err(LoadError::AlreadyLoaded(name));
        }

        self.ensure_jvm()?;
        let jvm = self.jvm.as_ref().unwrap();

        // Create the plugin object on this thread, convert to GlobalRef for the plugin thread.
        let plugin_global = {
            let mut env = jvm.attach_current_thread().map_err(|e| {
                LoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("JNI attach: {e}")))
            })?;
            let loader = create_url_classloader(&mut env, jar_path)?;
            let plugin_obj = instantiate_plugin(&mut env, &loader, &class_name)?;
            env.new_global_ref(&plugin_obj).map_err(|e| {
                LoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("global ref: {e}")))
            })?
        };

        // Register on the bus.
        let mailbox_rx = self.bus.register_plugin(&name);
        let sender = self.bus.sender_for(&name).unwrap();

        let jvm_ptr = jvm as *const JavaVM as usize;
        let thread_name = name.clone();
        let thread_plugin_name = name.clone();

        let handle = std::thread::Builder::new()
            .name(format!("plugin:{thread_name}"))
            .spawn(move || {
                let jvm = unsafe { &*(jvm_ptr as *const JavaVM) };
                java_plugin_thread(jvm, plugin_global, mailbox_rx, thread_plugin_name);
            })
            .map_err(LoadError::Io)?;

        self.plugins.insert(
            name,
            LoadedPlugin {
                meta: meta.clone(),
                sender,
                thread_handle: Some(handle),
            },
        );

        log::info!("jvm: loaded plugin: {} v{}", meta.name, meta.version);
        Ok(meta)
    }

    pub fn unload_plugin(&mut self, name: &str) -> Result<(), LoadError> {
        let mut plugin = self
            .plugins
            .remove(name)
            .ok_or_else(|| LoadError::NotLoaded(name.to_string()))?;

        if plugin.sender.try_send(PluginMail::Shutdown).is_err() {
            log::warn!("jvm plugin [{name}]: failed to send shutdown");
        }

        plugin.join();
        self.bus.unregister_plugin(name);
        log::info!("jvm: unloaded plugin: {name}");
        Ok(())
    }

    pub fn loaded_plugins(&self) -> Vec<&PluginMeta> {
        self.plugins.values().map(|p| &p.meta).collect()
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    pub fn loaded_count(&self) -> usize {
        self.plugins.len()
    }

    pub fn shutdown_all(&mut self) {
        let names: Vec<String> = self.plugins.keys().cloned().collect();
        for name in names {
            if let Err(e) = self.unload_plugin(&name) {
                log::error!("jvm: failed to unload {name}: {e}");
            }
        }
    }
}

impl Drop for JavaPluginManager {
    fn drop(&mut self) {
        if !self.plugins.is_empty() {
            self.shutdown_all();
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin thread
// ---------------------------------------------------------------------------

fn java_plugin_thread(
    jvm: &JavaVM,
    plugin: GlobalRef,
    mut mailbox: mpsc::Receiver<PluginMail>,
    plugin_name: String,
) {
    let mut env = match jvm.attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            log::error!("jvm [{plugin_name}]: failed to attach thread: {e}");
            return;
        }
    };

    // Call setup().
    if let Err(e) = env.call_method(&plugin, "setup", "()V", &[]) {
        log::error!("jvm [{plugin_name}]: setup failed: {e}");
        return;
    }
    log::info!("jvm [{plugin_name}]: setup complete");

    // Event loop.
    while let Some(mail) = mailbox.blocking_recv() {
        match mail {
            PluginMail::RenderRequest { reply } => {
                let json = call_render(&mut env, &plugin, &plugin_name);
                let _ = reply.send(json);
            }

            PluginMail::WidgetEvent { json } => {
                call_on_event(&mut env, &plugin, &json, &plugin_name);
            }

            PluginMail::BusEvent(msg) => {
                let event = conch_plugin_sdk::PluginEvent::BusEvent {
                    event_type: msg.event_type.clone(),
                    data: msg.data.clone(),
                };
                if let Ok(json) = serde_json::to_string(&event) {
                    call_on_event(&mut env, &plugin, &json, &plugin_name);
                }
            }

            PluginMail::BusQuery(req) => {
                let _ = req.reply.send(QueryResponse {
                    result: Err("Java plugins do not support queries yet".into()),
                });
            }

            PluginMail::Shutdown => {
                log::info!("jvm [{plugin_name}]: shutting down");
                break;
            }
        }
    }

    // Call teardown().
    if let Err(e) = env.call_method(&plugin, "teardown", "()V", &[]) {
        log::warn!("jvm [{plugin_name}]: teardown failed: {e}");
    }
    log::info!("jvm [{plugin_name}]: thread exiting");
}

fn call_render(env: &mut JNIEnv, plugin: &GlobalRef, plugin_name: &str) -> String {
    match env.call_method(plugin, "render", "()Ljava/lang/String;", &[]) {
        Ok(val) => match val.l() {
            Ok(obj) => {
                let jstr = JString::from(obj);
                env.get_string(&jstr)
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "[]".to_string())
            }
            Err(_) => "[]".to_string(),
        },
        Err(e) => {
            log::warn!("jvm [{plugin_name}]: render failed: {e}");
            // Clear any pending Java exception so subsequent calls work.
            let _ = env.exception_clear();
            "[]".to_string()
        }
    }
}

fn call_on_event(env: &mut JNIEnv, plugin: &GlobalRef, json: &str, plugin_name: &str) {
    let jstr = match env.new_string(json) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("jvm [{plugin_name}]: failed to create event string: {e}");
            return;
        }
    };
    if let Err(e) = env.call_method(plugin, "onEvent", "(Ljava/lang/String;)V", &[JValue::Object(&jstr)]) {
        log::warn!("jvm [{plugin_name}]: onEvent failed: {e}");
        let _ = env.exception_clear();
    }
}

// ---------------------------------------------------------------------------
// JNI helpers
// ---------------------------------------------------------------------------

/// Read `Plugin-Class` from a JAR's META-INF/MANIFEST.MF.
fn read_plugin_class_from_jar(jar_path: &Path) -> Result<String, LoadError> {
    let file = std::fs::File::open(jar_path)
        .map_err(|e| LoadError::Io(e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| LoadError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())))?;

    let manifest = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|_| LoadError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "JAR missing META-INF/MANIFEST.MF",
        )))?;

    let content = std::io::read_to_string(manifest)
        .map_err(|e| LoadError::Io(e))?;

    for line in content.lines() {
        if let Some(class) = line.strip_prefix("Plugin-Class:") {
            return Ok(class.trim().to_string());
        }
    }

    Err(LoadError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("no Plugin-Class in manifest of {}", jar_path.display()),
    )))
}

/// Create a URLClassLoader for a JAR file.
fn create_url_classloader<'a>(
    env: &mut JNIEnv<'a>,
    jar_path: &Path,
) -> Result<JObject<'a>, LoadError> {
    let abs_path = jar_path.canonicalize().map_err(LoadError::Io)?;
    let uri_str = format!("file://{}", abs_path.display());

    // new java.net.URL(uriStr)
    let url_str = env.new_string(&uri_str).map_err(jni_err)?;
    let url = env
        .new_object("java/net/URL", "(Ljava/lang/String;)V", &[JValue::Object(&url_str)])
        .map_err(jni_err)?;

    // Create URL[] { url }
    let url_class = env.find_class("java/net/URL").map_err(jni_err)?;
    let url_array = env.new_object_array(1, url_class, &url).map_err(jni_err)?;

    // new URLClassLoader(urls)
    let loader = env
        .new_object(
            "java/net/URLClassLoader",
            "([Ljava/net/URL;)V",
            &[JValue::Object(&url_array)],
        )
        .map_err(jni_err)?;

    Ok(loader)
}

/// Load and instantiate the plugin class from a URLClassLoader.
fn instantiate_plugin<'a>(
    env: &mut JNIEnv<'a>,
    loader: &JObject<'a>,
    class_name: &str,
) -> Result<JObject<'a>, LoadError> {
    // loader.loadClass(className)
    let jname = env.new_string(class_name).map_err(jni_err)?;
    let cls_obj = env
        .call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&jname)],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;

    let cls = JClass::from(cls_obj);

    // cls.getDeclaredConstructor().newInstance()
    let constructor = env
        .call_method(
            &cls,
            "getDeclaredConstructor",
            "([Ljava/lang/Class;)Ljava/lang/reflect/Constructor;",
            &[JValue::Object(&JObject::null())],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;

    let instance = env
        .call_method(
            &constructor,
            "newInstance",
            "([Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::Object(&JObject::null())],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;

    Ok(instance)
}

/// Read PluginInfo fields from a Java PluginInfo object.
fn read_plugin_info(env: &mut JNIEnv, info: &JObject) -> Result<PluginMeta, LoadError> {
    let name = get_string_field(env, info, "name")?;
    let description = get_string_field(env, info, "description")?;
    let version = get_string_field(env, info, "version")?;
    let plugin_type_str = get_string_field(env, info, "pluginType")?;
    let panel_location_str = get_string_field(env, info, "panelLocation")?;

    let plugin_type = match plugin_type_str.as_str() {
        "panel" => conch_plugin_sdk::PluginType::Panel,
        _ => conch_plugin_sdk::PluginType::Action,
    };
    let panel_location = match panel_location_str.as_str() {
        "left" => conch_plugin_sdk::PanelLocation::Left,
        "right" => conch_plugin_sdk::PanelLocation::Right,
        "bottom" => conch_plugin_sdk::PanelLocation::Bottom,
        _ => conch_plugin_sdk::PanelLocation::None,
    };

    Ok(PluginMeta {
        name,
        description,
        version,
        plugin_type,
        panel_location,
        dependencies: vec![],
    })
}

fn get_string_field(env: &mut JNIEnv, obj: &JObject, field: &str) -> Result<String, LoadError> {
    let val = env
        .get_field(obj, field, "Ljava/lang/String;")
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;
    let jstr = JString::from(val);
    env.get_string(&jstr)
        .map(|s| s.to_string_lossy().into_owned())
        .map_err(jni_err)
}

// ---------------------------------------------------------------------------
// JNI native method registration
// ---------------------------------------------------------------------------

/// Register native method implementations for `conch.plugin.HostApi`.
fn register_host_natives(env: &mut JNIEnv) -> Result<(), LoadError> {
    let class = env.find_class("conch/plugin/HostApi").map_err(jni_err)?;

    let methods: &[NativeMethod] = &[
        NativeMethod {
            name: "log".into(),
            sig: "(ILjava/lang/String;)V".into(),
            fn_ptr: native_host_log as *mut std::ffi::c_void,
        },
        NativeMethod {
            name: "registerMenuItem".into(),
            sig: "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V".into(),
            fn_ptr: native_host_register_menu_item as *mut std::ffi::c_void,
        },
    ];

    env.register_native_methods(class, methods).map_err(jni_err)?;
    log::info!("jvm: registered HostApi native methods");
    Ok(())
}

/// JNI implementation of `HostApi.log(int level, String message)`.
extern "system" fn native_host_log(mut env: JNIEnv, _class: JClass, level: jint, message: JString) {
    let host_api = HOST_API_PTR.load(Ordering::Acquire);
    if host_api.is_null() {
        return;
    }

    let msg: String = match env.get_string(&message) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return,
    };

    let c_msg = CString::new(msg).unwrap_or_default();
    unsafe { ((*host_api).log)(level as u8, c_msg.as_ptr()) };
}

/// JNI implementation of `HostApi.registerMenuItem(String menu, String label, String action)`.
extern "system" fn native_host_register_menu_item(
    mut env: JNIEnv,
    _class: JClass,
    menu: JString,
    label: JString,
    action: JString,
) {
    let host_api = HOST_API_PTR.load(Ordering::Acquire);
    if host_api.is_null() {
        return;
    }

    let menu_str = match env.get_string(&menu) { Ok(s) => s.to_string_lossy().into_owned(), Err(_) => return };
    let label_str = match env.get_string(&label) { Ok(s) => s.to_string_lossy().into_owned(), Err(_) => return };
    let action_str = match env.get_string(&action) { Ok(s) => s.to_string_lossy().into_owned(), Err(_) => return };

    let c_menu = CString::new(menu_str).unwrap_or_default();
    let c_label = CString::new(label_str).unwrap_or_default();
    let c_action = CString::new(action_str).unwrap_or_default();

    unsafe {
        ((*host_api).register_menu_item)(
            c_menu.as_ptr(),
            c_label.as_ptr(),
            c_action.as_ptr(),
            std::ptr::null(),
        );
    }
}

// ---------------------------------------------------------------------------
// Error conversion
// ---------------------------------------------------------------------------

fn jni_err<E: std::fmt::Display>(e: E) -> LoadError {
    LoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_manifest_from_test_jar() {
        // This test only works if the java-hello plugin has been built.
        let jar = PathBuf::from("../../plugins/java-hello/build/hello-plugin.jar");
        if !jar.exists() {
            return; // Skip if not built.
        }
        let class = read_plugin_class_from_jar(&jar).unwrap();
        assert_eq!(class, "conch.plugin.hello.HelloPlugin");
    }
}
