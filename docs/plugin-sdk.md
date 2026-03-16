# Conch Plugin SDK

Conch supports three plugin tiers, each suited to different use cases:

| Tier | Language | Use Case | Build Step |
|------|----------|----------|------------|
| **Native** | Rust, C, Go | Performance-critical, session backends (SSH, SFTP) | Compile to `.dylib`/`.so`/`.dll` |
| **Java** | Java, Kotlin, Scala, Groovy | Community plugins, rich UI, familiar ecosystem | Compile to `.jar` |
| **Lua** | Lua | Quick scripts, personal automation, no build step | Single `.lua` file |

## Table of Contents

- [Java Plugins](#java-plugins)
  - [Quick Start](#java-quick-start)
  - [Project Setup (Gradle)](#project-setup-gradle)
  - [ConchPlugin Interface](#conchplugin-interface)
  - [HostApi Reference](#java-hostapi)
  - [Widget Builder](#widget-builder)
  - [Handling Events](#handling-events-java)
  - [Panel Plugins](#panel-plugins-java)
- [Native Plugins](#native-plugins)
  - [Architecture Overview](#architecture-overview)
  - [Quick Start (Rust)](#quick-start-rust)
  - [Required Exports](#required-exports)
  - [HostApi Reference](#hostapi-reference)
  - [Session Backends](#session-backends)
  - [Examples in Other Languages](#examples-in-other-languages)
- [Lua Plugins](#lua-plugins)
  - [Quick Start](#lua-quick-start)
  - [Lua API Reference](#lua-api-reference)
  - [Panel Widget Functions](#panel-widget-functions)
  - [Net API](#net-api)
- [Widget System](#widget-system)
- [Widget Events](#widget-events)
- [Plugin Events](#plugin-events)
- [Inter-Plugin Communication](#inter-plugin-communication)
- [Plugin Search Paths](#plugin-search-paths)

---

## Java Plugins

Java plugins are JAR files loaded by an embedded JVM. Any JVM language works (Java, Kotlin, Scala, Groovy). The SDK JAR is embedded in the Conch binary — no external files needed.

This is the recommended tier for community plugins. If you've written Bukkit/Paper plugins for Minecraft, this will feel familiar.

### Java Quick Start

**1. Create a Gradle project:**

```groovy
// build.gradle
plugins {
    id 'java'
}

dependencies {
    compileOnly files('path/to/conch-plugin-sdk.jar')
}

jar {
    manifest {
        attributes 'Plugin-Class': 'com.example.MyPlugin'
    }
}
```

> Download `conch-plugin-sdk.jar` from the [Releases](https://github.com/an0nn30/rusty_conch/releases) page, or build it locally with `make java-sdk`.

**2. Implement `ConchPlugin`:**

```java
package com.example;

import conch.plugin.ConchPlugin;
import conch.plugin.HostApi;
import conch.plugin.PluginInfo;

public class MyPlugin implements ConchPlugin {

    @Override
    public PluginInfo getInfo() {
        return new PluginInfo(
            "My Plugin",
            "A simple Java plugin",
            "1.0.0"
        );
    }

    @Override
    public void setup() {
        HostApi.info("My plugin loaded!");
        HostApi.registerMenuItem("Tools", "Do Thing", "do_thing");
    }

    @Override
    public void onEvent(String eventJson) {
        if (eventJson.contains("do_thing")) {
            HostApi.info("Thing was done!");
        }
    }

    @Override
    public String render() {
        return "[]"; // No panel widgets
    }

    @Override
    public void teardown() {
        HostApi.info("My plugin unloaded");
    }
}
```

**3. Build and install:**

```bash
gradle build
cp build/libs/my-plugin.jar ~/.config/conch/plugins/
```

Open Conch, go to the Plugin Manager, and load your plugin.

### Project Setup (Gradle)

```groovy
plugins {
    id 'java'
}

group = 'com.example'
version = '1.0.0'

java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

dependencies {
    // The SDK is provided by Conch at runtime — don't bundle it.
    compileOnly files('path/to/conch-plugin-sdk.jar')
}

jar {
    manifest {
        // REQUIRED: tells Conch which class to load.
        attributes 'Plugin-Class': 'com.example.MyPlugin'
    }
}
```

> **Tip: Bundling dependencies into a fat JAR.** Conch loads your plugin as a
> single JAR — external dependencies (like Gson) must be bundled inside it.
> Use the Shadow plugin to create a fat JAR:
>
> ```groovy
> plugins {
>     id 'java'
>     id 'com.github.johnrengelman.shadow' version '8.1.1'
> }
>
> dependencies {
>     compileOnly files('libs/conch-plugin-sdk.jar')  // provided by Conch
>     implementation 'com.google.code.gson:gson:2.11.0'  // bundled into JAR
> }
>
> shadowJar {
>     archiveClassifier.set('')  // replace the default jar
>     manifest {
>         attributes 'Plugin-Class': 'com.example.MyPlugin'
>     }
>     // Avoid META-INF merge conflicts.
>     exclude 'META-INF/*.SF', 'META-INF/*.DSA', 'META-INF/*.RSA'
>     mergeServiceFiles()
> }
> ```
>
> Build with `gradle shadowJar`. The output JAR in `build/libs/` will
> contain your code + all `implementation` dependencies. With the
> `archiveClassifier.set('')` config above, `gradle build` also produces
> the fat JAR automatically.

**Maven:**

```xml
<dependencies>
    <dependency>
        <groupId>conch.plugin</groupId>
        <artifactId>conch-plugin-sdk</artifactId>
        <version>0.3.0</version>
        <scope>system</scope>
        <systemPath>${project.basedir}/libs/conch-plugin-sdk.jar</systemPath>
    </dependency>
</dependencies>

<build>
    <plugins>
        <plugin>
            <groupId>org.apache.maven.plugins</groupId>
            <artifactId>maven-jar-plugin</artifactId>
            <configuration>
                <archive>
                    <manifestEntries>
                        <Plugin-Class>com.example.MyPlugin</Plugin-Class>
                    </manifestEntries>
                </archive>
            </configuration>
        </plugin>
    </plugins>
</build>
```

### ConchPlugin Interface

Every Java plugin must implement `conch.plugin.ConchPlugin`:

| Method | Description |
|--------|-------------|
| `PluginInfo getInfo()` | Return plugin metadata (name, version, type, panel location) |
| `void setup()` | Called once on plugin load. Register menu items, initialize state. |
| `void onEvent(String eventJson)` | Handle events — menu clicks, widget interactions, bus events. |
| `String render()` | Return widget tree as JSON array. Called every frame for panel plugins. |
| `void teardown()` | Clean up resources before unload. |

#### Plugin Types

```java
// Action plugin — no panel, interacts via menu items and events.
new PluginInfo("My Tool", "Does things", "1.0.0");

// Panel plugin — renders widgets in a sidebar or bottom panel.
new PluginInfo("My Panel", "Shows info", "1.0.0", "panel", "bottom");
```

Panel locations: `"left"`, `"right"`, `"bottom"`, `"none"`.

### Java HostApi

Static methods on `conch.plugin.HostApi`.

**Logging:**

| Method | Description |
|--------|-------------|
| `log(int level, String message)` | Log a message (0=trace, 1=debug, 2=info, 3=warn, 4=error) |
| `trace(String message)` | Log at TRACE |
| `debug(String message)` | Log at DEBUG |
| `info(String message)` | Log at INFO |
| `warn(String message)` | Log at WARN |
| `error(String message)` | Log at ERROR |

**Menu Items:**

| Method | Description |
|--------|-------------|
| `registerMenuItem(String menu, String label, String action)` | Add a menu item |
| `registerMenuItemWithKeybind(String menu, String label, String action, String keybind)` | Add a menu item with keyboard shortcut (e.g. `"cmd+shift+j"`) |

**Notifications & Status:**

| Method | Description |
|--------|-------------|
| `notify(String title, String body, String level, int durationMs)` | Show a toast notification (level: `"info"`, `"success"`, `"warn"`, `"error"`) |
| `notify(String title, String body, String level)` | Show notification with default duration |
| `setStatus(String text, int level, float progress)` | Update status bar (progress: 0.0–1.0, or negative to hide) |

**Dialogs (blocking):**

| Method | Description |
|--------|-------------|
| `prompt(String message, String defaultValue)` | Show a text input dialog, returns entered text or null |
| `prompt(String message)` | Prompt with no default value |
| `confirm(String message)` | Show OK/Cancel dialog, returns true/false |
| `alert(String title, String message)` | Show an alert dialog |
| `showError(String title, String message)` | Show an error dialog |
| `showForm(String formJson)` | Show a multi-field form dialog (returns JSON result or null) |

**Clipboard:**

| Method | Description |
|--------|-------------|
| `clipboardSet(String text)` | Copy text to system clipboard |
| `clipboardGet()` | Get clipboard text (returns null if unavailable) |

**Config (persistent per-plugin storage):**

| Method | Description |
|--------|-------------|
| `getConfig(String key)` | Read a config value (returns JSON string or null) |
| `setConfig(String key, String value)` | Write a config value (null to delete) |

**Inter-Plugin Communication:**

| Method | Description |
|--------|-------------|
| `subscribe(String eventType)` | Subscribe to bus events from other plugins |
| `publishEvent(String eventType, String dataJson)` | Publish a bus event |

**Terminal / Session:**

| Method | Description |
|--------|-------------|
| `writeToPty(String text)` | Write text to the focused terminal (include `\n` for Enter) |
| `newTab(String command, boolean plain)` | Open a new tab (plain=true bypasses terminal.shell config) |
| `newTab()` | Open a new tab with default shell |
| `newPlainTab(String command)` | Open a plain shell tab and run a command |

### Widget Builder

The `conch.plugin.Widgets` class provides a fluent builder for constructing widget trees without writing raw JSON:

```java
@Override
public String render() {
    return new Widgets()
        .heading("System Info")
        .separator()
        .keyValue("OS", System.getProperty("os.name"))
        .keyValue("Java", System.getProperty("java.version"))
        .keyValue("Arch", System.getProperty("os.arch"))
        .separator()
        .button("refresh", "Refresh")
        .toJson();
}
```

Available builder methods:

| Method | Description |
|--------|-------------|
| `heading(text)` | Section heading |
| `label(text)` / `label(text, style)` | Text label with optional style |
| `text(text)` | Monospace text |
| `keyValue(key, value)` | Key-value display row |
| `separator()` | Horizontal rule |
| `spacer()` / `spacer(size)` | Flexible or fixed spacer |
| `badge(text, variant)` | Status badge (info/success/warn/error) |
| `progress(id, fraction, label)` | Progress bar |
| `button(id, label)` / `button(id, label, icon)` | Clickable button |
| `textInput(id, value, hint)` | Single-line text input |
| `checkbox(id, label, checked)` | Checkbox toggle |
| `horizontal(children)` | Horizontal layout |
| `vertical(children)` | Vertical layout |
| `scrollArea(maxHeight, children)` | Scrollable container |
| `raw(json)` | Insert raw JSON for unsupported widget types |

### Handling Events (Java)

Events arrive as JSON strings in `onEvent()`. Use `String.contains()` for simple matching:

```java
@Override
public void onEvent(String eventJson) {
    // Menu action
    if (eventJson.contains("\"my_action\"")) {
        HostApi.info("Menu item clicked!");
    }

    // Button click
    if (eventJson.contains("\"button_click\"") && eventJson.contains("\"my_button\"")) {
        HostApi.info("Button clicked!");
    }
}
```

For structured parsing, add Gson to your project (`implementation 'com.google.code.gson:gson:2.11.0'` in Gradle):

```java
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

JsonObject event = JsonParser.parseString(eventJson).getAsJsonObject();
if (event.get("kind").getAsString().equals("menu_action")) {
    String action = event.get("action").getAsString();
    // ...
}
```

### Panel Plugins (Java)

To create a panel plugin, set `pluginType` to `"panel"` and specify a location:

```java
@Override
public PluginInfo getInfo() {
    return new PluginInfo(
        "My Panel",
        "Shows useful info",
        "1.0.0",
        "panel",    // Plugin type
        "bottom"    // Panel location: left, right, or bottom
    );
}

@Override
public String render() {
    // This is called every frame. Return your widget tree.
    return new Widgets()
        .heading("My Panel")
        .label("Hello from Java!")
        .toJson();
}
```

The panel is automatically registered when the plugin loads. When multiple plugins register at the same location, they appear as tabs.

---

## Native Plugins

Native plugins are compiled shared libraries (`.dylib` on macOS, `.so` on Linux, `.dll` on Windows). They communicate with the host through a C ABI, making the SDK language-agnostic.

Use native plugins for performance-critical functionality like terminal session backends (SSH, SFTP, serial).

### Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                  Conch Host App                  │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐  │
│  │ PluginBus│  │  Panel   │  │  HostApi       │  │
│  │ (IPC)    │  │ Registry │  │  (vtable)      │  │
│  └────┬─────┘  └────┬─────┘  └───────┬───────┘  │
│       │              │                │          │
└───────┼──────────────┼────────────────┼──────────┘
        │              │                │
        │   ┌──────────┴────────────────┘
        │   │  C ABI boundary
        │   │
   ┌────┴───┴──────────────────────────────────┐
   │            Plugin (.dylib/.so/.dll)        │
   │                                            │
   │  conch_plugin_info()     → metadata        │
   │  conch_plugin_setup()    → state pointer   │
   │  conch_plugin_event()    ← JSON events     │
   │  conch_plugin_render()   → JSON widgets    │
   │  conch_plugin_query()    ← RPC calls       │
   │  conch_plugin_teardown() → cleanup         │
   └────────────────────────────────────────────┘
```

Each plugin runs on its own thread. The host sends messages (render requests, events, queries) via a channel, and the plugin responds through exported functions.

### Quick Start (Rust)

**Cargo.toml:**
```toml
[package]
name = "my-plugin"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
conch_plugin_sdk = { path = "../../crates/conch_plugin_sdk" }
serde_json = "1"
```

**src/lib.rs:**
```rust
use std::ffi::CString;
use conch_plugin_sdk::{
    widgets::{PluginEvent, Widget, WidgetEvent},
    HostApi, PanelHandle, PanelLocation, PluginInfo, PluginType,
};

struct MyPlugin {
    api: &'static HostApi,
    panel: PanelHandle,
    counter: u64,
}

impl MyPlugin {
    fn new(api: &'static HostApi) -> Self {
        let msg = CString::new("My plugin loaded!").unwrap();
        (api.log)(2, msg.as_ptr());

        let name = CString::new("My Panel").unwrap();
        let panel = (api.register_panel)(PanelLocation::Left, name.as_ptr(), std::ptr::null());

        Self { api, panel, counter: 0 }
    }

    fn handle_event(&mut self, event: PluginEvent) {
        if let PluginEvent::Widget(WidgetEvent::ButtonClick { id }) = event {
            if id == "increment" {
                self.counter += 1;
            }
        }
    }

    fn render(&self) -> Vec<Widget> {
        vec![
            Widget::heading("My Plugin"),
            Widget::KeyValue {
                key: "Count".into(),
                value: self.counter.to_string(),
            },
            Widget::button("increment", "Add One"),
        ]
    }

    fn handle_query(&self, method: &str, args: serde_json::Value) -> serde_json::Value {
        match method {
            "get_count" => serde_json::json!({ "count": self.counter }),
            _ => serde_json::json!({ "error": "unknown method" }),
        }
    }
}

conch_plugin_sdk::declare_plugin!(
    info: PluginInfo {
        name: c"My Plugin".as_ptr(),
        description: c"A simple counter plugin".as_ptr(),
        version: c"0.1.0".as_ptr(),
        plugin_type: PluginType::Panel,
        panel_location: PanelLocation::Left,
        dependencies: std::ptr::null(),
        num_dependencies: 0,
    },
    state: MyPlugin,
    setup: |api| MyPlugin::new(api),
    event: |state, event| state.handle_event(event),
    render: |state| state.render(),
    query: |state, method, args| state.handle_query(method, args),
);
```

Build with `cargo build` and the `.dylib`/`.so` is discovered automatically.

### Required Exports

Every native plugin must export six C-ABI functions:

| Symbol | Signature | Purpose |
|--------|-----------|---------|
| `conch_plugin_info` | `() -> PluginInfo` | Return static metadata |
| `conch_plugin_setup` | `(*const HostApi) -> *mut c_void` | Initialize plugin state |
| `conch_plugin_event` | `(*mut c_void, *const c_char, usize)` | Handle incoming events (JSON) |
| `conch_plugin_render` | `(*mut c_void) -> *const c_char` | Return widget tree (JSON) |
| `conch_plugin_teardown` | `(*mut c_void)` | Cleanup and free state |
| `conch_plugin_query` | `(*mut c_void, *const c_char, *const c_char, usize) -> *mut c_char` | Handle RPC queries |

### HostApi Reference

The `HostApi` is a `#[repr(C)]` struct of function pointers passed to `conch_plugin_setup`. See the full reference in the [HostApi section below](#hostapi-full-reference).

### Session Backends

Native plugins can provide terminal session backends (SSH, serial, telnet). The plugin provides a vtable of callbacks for write, resize, and shutdown. See the SSH plugin (`plugins/conch-ssh/`) for a complete example.

### Examples in Other Languages

Native plugins can be written in any language that produces C-compatible shared libraries. See the C and Go examples in the [full native plugin documentation](https://github.com/an0nn30/rusty_conch/blob/v2/docs/plugin-sdk-native.md).

---

## Lua Plugins

Lua plugins are single `.lua` files — no compilation, no project setup. Drop a file in the plugins directory and it's discovered immediately. Good for quick personal scripts and automation.

### Lua Quick Start

Create a file in your plugins directory (e.g., `~/.config/conch/plugins/my-script.lua`):

```lua
-- plugin-name: My Script
-- plugin-description: A quick automation script
-- plugin-type: action
-- plugin-version: 1.0.0

function setup()
    app.log("info", "My script loaded!")
    app.register_menu_item("Tools", "Run My Script", "run_script")
end

function on_event(event)
    if type(event) == "table" and event.action == "run_script" then
        app.log("info", "Script executed!")
    end
end

function render()
    return "[]"
end

function teardown()
    app.log("info", "My script unloaded")
end
```

Metadata is declared in comments at the top of the file. No build step — the host reads and executes the Lua source directly.

### Lua API Reference

All functions are on the `app` global table.

| Function | Description |
|----------|-------------|
| `app.log(level, message)` | Log a message (level: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`) |
| `app.register_menu_item(menu, label, action, keybind?)` | Add a menu item (keybind is optional, e.g. `"cmd+shift+j"`) |
| `app.register_service(name)` | Register as a named service for inter-plugin queries |
| `app.subscribe(event_type)` | Subscribe to bus events from other plugins |
| `app.publish(event_type, data)` | Publish a bus event |
| `app.get_config(key)` | Read a persisted config value (returns string or nil) |
| `app.set_config(key, value)` | Write a persisted config value |
| `app.notify(title, body, level?, duration_ms?)` | Show a toast notification (level: `"info"`, `"success"`, `"warn"`, `"error"`) |
| `app.clipboard(text)` | Copy text to system clipboard |
| `app.clipboard_get()` | Get clipboard text (returns string or nil) |
| `app.query_plugin(target, method, args?)` | Send a direct RPC query to another plugin (returns string or nil) |
| `ui.prompt(message, default?)` | Show a blocking text input dialog, returns string or nil |
| `ui.confirm(message)` | Show a blocking OK/Cancel dialog, returns boolean |
| `ui.alert(title, message)` | Show a blocking alert dialog |
| `ui.error(title, message)` | Show a blocking error dialog |
| `ui.form(title, fields)` | Show a multi-field form dialog, returns table or nil |
| `session.exec(command)` | Run a shell command locally, returns `{stdout, stderr, exit_code, status}` |
| `session.write(text)` | Write text to the focused terminal's PTY (include `\n` for Enter) |
| `session.new_tab(command?, plain?)` | Open a new tab; `plain=true` bypasses terminal.shell config |
| `session.current()` | Get info about the active session (`{platform, type}`) |
| `session.platform` | Get the current OS platform string |

**Log levels (Lua uses strings, not integers):**

| Value | Level |
|-------|-------|
| `"trace"` | Trace |
| `"debug"` | Debug |
| `"info"` | Info |
| `"warn"` | Warn |
| `"error"` | Error |

### Net API

The `net` global table provides basic networking utilities.

| Function | Description |
|----------|-------------|
| `net.time()` | Returns the current Unix timestamp as a float (seconds since epoch) |
| `net.resolve(hostname)` | DNS lookup — returns an array of IP address strings (empty array on failure) |
| `net.scan(host, ports, timeout_ms?, concurrency?)` | TCP port scan — returns an array of `{port, open}` tables for open ports |

### Panel Widget Functions

For Lua panel plugins (`plugin-type: panel`), the `ui` table provides `panel_*` functions that build a widget tree imperatively during `render()`. Instead of returning JSON, call these functions and they accumulate widgets automatically.

```lua
-- plugin-type: panel
-- plugin-panel-location: left

function render()
    ui.panel_heading("My Panel")
    ui.panel_separator()
    ui.panel_label("Hello from Lua!")
    ui.panel_kv("Status", "OK")
    ui.panel_button("refresh", "Refresh")
end
```

**Display widgets:**

| Function | Description |
|----------|-------------|
| `ui.panel_heading(text)` | Section heading |
| `ui.panel_label(text, style?)` | Text label (style: `"secondary"`, `"muted"`, `"accent"`, `"warn"`, `"error"`) |
| `ui.panel_text(text)` | Monospace text |
| `ui.panel_scroll_text(id, text, max_height?)` | Scrollable text area |
| `ui.panel_kv(key, value)` | Key-value display row |
| `ui.panel_separator()` | Horizontal rule |
| `ui.panel_spacer(size?)` | Flexible or fixed spacer |
| `ui.panel_icon_label(icon, text, style?)` | Icon + text label |
| `ui.panel_badge(text, variant)` | Status badge (info/success/warn/error) |
| `ui.panel_progress(id, fraction, label?)` | Progress bar (0.0--1.0) |
| `ui.panel_image(id?, src, width?, height?)` | Image widget |

**Interactive widgets:**

| Function | Description |
|----------|-------------|
| `ui.panel_button(id, label, icon?)` | Clickable button |
| `ui.panel_text_input(id, value, hint?)` | Single-line text input |
| `ui.panel_text_edit(id, value, hint?, lines?)` | Multi-line text editor |
| `ui.panel_checkbox(id, label, checked)` | Checkbox toggle |
| `ui.panel_combobox(id, selected, options)` | Dropdown select |

**Complex widgets:**

| Function | Description |
|----------|-------------|
| `ui.panel_table(columns, rows)` | Data table |
| `ui.panel_tree(id, nodes, selected?)` | Hierarchical tree view |
| `ui.panel_toolbar(id?, items)` | Toolbar with buttons, separators, inputs |
| `ui.panel_path_bar(id, segments)` | Breadcrumb path bar |
| `ui.panel_tabs(id, active, tabs)` | Tabbed container |

**Layout containers (take a function argument for children):**

| Function | Description |
|----------|-------------|
| `ui.panel_horizontal(func, spacing?)` | Horizontal layout |
| `ui.panel_vertical(func, spacing?)` | Vertical layout |
| `ui.panel_scroll_area(func, max_height?)` | Scrollable container |
| `ui.panel_drop_zone(id, label, func?)` | Drag-and-drop target |

**Utility:**

| Function | Description |
|----------|-------------|
| `ui.panel_clear()` | Clear all accumulated widgets |

---

## Widget System

All plugin tiers share the same widget system. Plugins return a JSON array of widget objects, and the host renders them using egui.

### Layout Widgets

| Widget | Fields | Description |
|--------|--------|-------------|
| `horizontal` | `id?`, `children`, `spacing?` | Horizontal layout |
| `vertical` | `id?`, `children`, `spacing?` | Vertical layout |
| `split_pane` | `id`, `direction`, `ratio`, `resizable`, `left`, `right` | Resizable split |
| `scroll_area` | `id?`, `max_height?`, `children` | Scrollable region |
| `tabs` | `id`, `active`, `tabs: [{label, children}]` | Tabbed container |

### Display Widgets

| Widget | Fields | Description |
|--------|--------|-------------|
| `heading` | `text` | Section heading |
| `label` | `text`, `style?` | Styled text (secondary/muted/accent/warn/error) |
| `text` | `text` | Monospace text |
| `scroll_text` | `id`, `text`, `max_height?` | Scrollable log output |
| `key_value` | `key`, `value` | Key-value row |
| `separator` | — | Horizontal rule |
| `spacer` | `size?` | Spacing |
| `badge` | `text`, `variant` | Status badge (info/success/warn/error) |
| `progress` | `id`, `fraction`, `label?` | Progress bar (0.0–1.0) |

### Interactive Widgets

| Widget | Fields | Description |
|--------|--------|-------------|
| `button` | `id`, `label`, `icon?`, `enabled?` | Clickable button |
| `text_input` | `id`, `value`, `hint?`, `submit_on_enter?` | Single-line text input |
| `text_edit` | `id`, `value`, `hint?`, `lines?` | Multi-line editor |
| `checkbox` | `id`, `label`, `checked` | Toggle |
| `combo_box` | `id`, `selected`, `options: [{value, label}]` | Dropdown |

### Complex Widgets

| Widget | Fields | Description |
|--------|--------|-------------|
| `toolbar` | `id?`, `items` | Button/separator/input toolbar |
| `path_bar` | `id`, `segments` | Breadcrumb path |
| `tree_view` | `id`, `nodes`, `selected?` | Hierarchical tree |
| `table` | `id`, `columns`, `rows`, `sort_column?`, `sort_ascending?`, `selected_row?` | Data table |
| `drop_zone` | `id`, `label`, `children` | Drag-and-drop target |
| `context_menu` | `child`, `items` | Right-click menu wrapper |

---

## Widget Events

When a user interacts with a widget, the plugin receives an event:

| Event | Fields | Trigger |
|-------|--------|---------|
| `button_click` | `id` | Button pressed |
| `text_input_changed` | `id`, `value` | Text input changed |
| `text_input_submit` | `id`, `value` | Enter pressed |
| `checkbox_changed` | `id`, `checked` | Checkbox toggled |
| `combo_box_changed` | `id`, `value` | Dropdown changed |
| `tree_select` | `id`, `node_id` | Tree node selected |
| `tree_activate` | `id`, `node_id` | Tree node double-clicked |
| `tree_context_menu` | `id`, `node_id`, `action` | Tree context menu action |
| `table_select` | `id`, `row_id` | Table row selected |
| `table_sort` | `id`, `column`, `ascending` | Column header clicked |
| `tab_changed` | `id`, `active` | Tab switched |
| `path_bar_navigate` | `id`, `segment_index` | Path segment clicked |

---

## Plugin Events

The top-level event envelope delivered to plugins:

```json
// Menu action (user clicked a registered menu item)
{ "kind": "menu_action", "action": "do_something" }

// Widget interaction
{ "kind": "widget", "type": "button_click", "id": "my_button" }

// Bus event from another plugin
{ "kind": "bus_event", "event_type": "ssh.connected", "data": { "host": "..." } }

// Theme changed
{ "kind": "theme_changed", "theme_json": "{...}" }
```

**Native/Java** plugins receive these as JSON strings and must parse them.

**Lua** plugins receive a **native Lua table** — the host automatically parses the JSON before calling `on_event()`. Access fields directly:

```lua
function on_event(event)
    -- event is a Lua table, NOT a JSON string.
    if event.kind == "menu_action" then
        local action = event.action
        app.log("info", "Menu action: " .. action)
    elseif event.kind == "widget" then
        if event.type == "button_click" then
            app.log("info", "Button clicked: " .. event.id)
        end
    elseif event.kind == "bus_event" then
        app.log("info", "Bus event: " .. event.event_type)
    end
end
```

> **Tip:** Use `type(event) == "table"` as a guard if you want to be defensive.

---

## Form Dialogs

All plugin tiers (Java, Lua, Native) share the same form JSON format. A form descriptor has a `title` and an array of `fields`. The dialog blocks until the user submits or cancels.

### Form Field Types

| Type | Fields | Description |
|------|--------|-------------|
| `text` | `id`, `label`, `value?`, `hint?` | Single-line text input |
| `password` | `id`, `label`, `value?` | Masked password input |
| `number` | `id`, `label`, `value?` | Numeric input |
| `combo` | `id`, `label`, `options[]`, `value?` | Dropdown select |
| `checkbox` | `id`, `label`, `value?` | Boolean toggle |
| `host_port` | `host_id`, `port_id`, `label`, `host_value?`, `port_value?` | Host + port on one row |
| `file_picker` | `id`, `label`, `value?`, `start_dir?` | Text input with Browse button |
| `collapsible` | `label`, `expanded?`, `fields[]` | Collapsible section with nested fields |
| `separator` | — | Horizontal rule |
| `label` | `text` | Read-only text |

### Example: Encrypt/Decrypt Form

This JSON works identically across Java, Lua, and Native plugins:

```json
{
    "title": "Encrypt / Decrypt",
    "fields": [
        {
            "type": "combo",
            "id": "mode",
            "label": "Operation",
            "options": ["Encrypt", "Decrypt"],
            "value": "Encrypt"
        },
        {
            "type": "password",
            "id": "key",
            "label": "Secret Key"
        },
        {
            "type": "text",
            "id": "input_text",
            "label": "Input Text",
            "hint": "Text to encrypt or decrypt..."
        }
    ]
}
```

> **Tip:** Use `"type": "password"` for any field that should be masked (keys, tokens, secrets).

**Java usage:**

```java
String formJson = """
    {
        "title": "Encrypt / Decrypt",
        "fields": [
            {"type": "combo", "id": "mode", "label": "Operation",
             "options": ["Encrypt", "Decrypt"], "value": "Encrypt"},
            {"type": "password", "id": "key", "label": "Secret Key"},
            {"type": "text", "id": "input_text", "label": "Input Text",
             "hint": "Text to encrypt or decrypt..."}
        ]
    }
    """;

String result = HostApi.showForm(formJson);
if (result != null) {
    // result = {"mode":"Encrypt", "key":"my-secret", "input_text":"hello world"}
    // Requires Gson: implementation 'com.google.code.gson:gson:2.11.0'
    JsonObject obj = JsonParser.parseString(result).getAsJsonObject();
    String mode = obj.get("mode").getAsString();
    String key = obj.get("key").getAsString();
    String text = obj.get("input_text").getAsString();
}
```

**Lua usage:**

```lua
local result = ui.form("Encrypt / Decrypt", {
    { type = "combo", id = "mode", label = "Operation",
      options = {"Encrypt", "Decrypt"}, value = "Encrypt" },
    { type = "password", id = "key", label = "Secret Key" },
    { type = "text", id = "input_text", label = "Input Text",
      hint = "Text to encrypt or decrypt..." },
})

if result then
    local mode = result.mode       -- "Encrypt" or "Decrypt"
    local key = result.key         -- the secret key (was masked)
    local text = result.input_text -- the input text
end
```

**Native (Rust) usage:**

```rust
let form_json = serde_json::json!({
    "title": "Encrypt / Decrypt",
    "fields": [
        {"type": "combo", "id": "mode", "label": "Operation",
         "options": ["Encrypt", "Decrypt"], "value": "Encrypt"},
        {"type": "password", "id": "key", "label": "Secret Key"},
        {"type": "text", "id": "input_text", "label": "Input Text",
         "hint": "Text to encrypt or decrypt..."}
    ]
});
let json_str = form_json.to_string();
let c_json = CString::new(json_str.clone()).unwrap();
let result_ptr = (api.show_form)(c_json.as_ptr(), json_str.len());
// Parse result JSON or handle null (cancelled).
```

### Example: Connection Settings with Collapsible Advanced Options

```json
{
    "title": "Connection Settings",
    "fields": [
        {"type": "text", "id": "host", "label": "Hostname", "hint": "server.example.com"},
        {"type": "number", "id": "port", "label": "Port", "value": 22},
        {"type": "text", "id": "username", "label": "Username"},
        {"type": "combo", "id": "auth", "label": "Authentication",
         "options": ["Password", "SSH Key", "Agent"], "value": "Password"},
        {"type": "password", "id": "password", "label": "Password"},
        {"type": "separator"},
        {
            "type": "collapsible",
            "label": "Advanced Options",
            "expanded": false,
            "fields": [
                {"type": "file_picker", "id": "key_file", "label": "SSH Key File",
                 "start_dir": "~/.ssh"},
                {"type": "checkbox", "id": "compression", "label": "Enable compression",
                 "value": false},
                {"type": "number", "id": "timeout", "label": "Timeout (seconds)", "value": 30}
            ]
        }
    ]
}
```

---

## Inter-Plugin Communication

### Pub/Sub Events

Plugins can publish events that other plugins subscribe to:

```java
// Java — subscribe in setup(), receive in onEvent()
// Native — api.subscribe("ssh.connected"), api.publish_event(...)
// Lua — app.subscribe("ssh.connected"), app.publish(...)
```

### RPC Queries

Plugins can send direct queries to other plugins:

```java
// Native only (not yet available in Java/Lua)
char* result = api->query_plugin("SSH Manager", "get_sessions", "{}", len);
```

---

## Plugin Search Paths

The host scans these directories (in order):

1. `target/debug` and `target/release` (development)
2. `examples/plugins`
3. Executable directory and `plugins/` subdirectory
4. macOS: `Conch.app/Contents/Plugins/`
5. Linux: `/opt/conch/lib`, `/usr/lib/conch/plugins`
6. User config directory (`~/.config/conch/plugins/` or `~/Library/Application Support/conch/plugins/`)
7. Custom paths from `config.toml`: `[conch.plugins] search_paths = ["~/my-plugins"]`

**File extensions:**
- Native: `.dylib` (macOS), `.so` (Linux), `.dll` (Windows)
- Java: `.jar` (must have `Plugin-Class` in `META-INF/MANIFEST.MF`)
- Lua: `.lua` (must have metadata comments at the top)
