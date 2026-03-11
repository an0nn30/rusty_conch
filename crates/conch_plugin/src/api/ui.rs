use mlua::{Lua, Result as LuaResult, Value};

use super::{FormField, PanelWidget, PluginCommand, PluginContext, PluginResponse};

/// Register the `ui` table into the Lua state.
pub fn register(lua: &Lua, ctx: PluginContext) -> LuaResult<()> {
    let ui = lua.create_table()?;

    // ui.append(text) — append text to plugin output panel
    let ctx_append = ctx.clone();
    ui.set(
        "append",
        lua.create_function(move |_lua, text: String| {
            ctx_append.send_fire_and_forget(PluginCommand::UiAppend(text));
            Ok(())
        })?,
    )?;

    // ui.clear() — clear the plugin output panel
    let ctx_clear = ctx.clone();
    ui.set(
        "clear",
        lua.create_function(move |_lua, ()| {
            ctx_clear.send_fire_and_forget(PluginCommand::UiClear);
            Ok(())
        })?,
    )?;

    // ui.form(title, fields_table) — show a form dialog, returns table or nil
    let ctx_form = ctx.clone();
    ui.set(
        "form",
        lua.create_async_function(move |lua, (title, fields_table): (String, mlua::Table)| {
            let ctx = ctx_form.clone();
            async move {
                let fields = parse_form_fields(&fields_table)?;
                let resp = ctx
                    .send_command(PluginCommand::ShowForm { title, fields })
                    .await;
                match resp {
                    PluginResponse::FormResult(Some(map)) => {
                        let t = lua.create_table()?;
                        for (k, v) in map {
                            t.set(k, v)?;
                        }
                        Ok(Value::Table(t))
                    }
                    _ => Ok(Value::Nil),
                }
            }
        })?,
    )?;

    // ui.prompt(message) — show a text input prompt, returns string or nil
    let ctx_prompt = ctx.clone();
    ui.set(
        "prompt",
        lua.create_async_function(move |_lua, message: String| {
            let ctx = ctx_prompt.clone();
            async move {
                let resp = ctx
                    .send_command(PluginCommand::ShowPrompt { message })
                    .await;
                match resp {
                    PluginResponse::Output(s) => Ok(Value::String(
                        _lua.create_string(&s)?,
                    )),
                    _ => Ok(Value::Nil),
                }
            }
        })?,
    )?;

    // ui.confirm(message) — show a yes/no dialog, returns boolean
    let ctx_confirm = ctx.clone();
    ui.set(
        "confirm",
        lua.create_async_function(move |_lua, message: String| {
            let ctx = ctx_confirm.clone();
            async move {
                let resp = ctx
                    .send_command(PluginCommand::ShowConfirm { message })
                    .await;
                match resp {
                    PluginResponse::Bool(b) => Ok(Value::Boolean(b)),
                    _ => Ok(Value::Boolean(false)),
                }
            }
        })?,
    )?;

    // ui.alert(title, msg) — show an informational alert
    let ctx_alert = ctx.clone();
    ui.set(
        "alert",
        lua.create_async_function(move |_lua, (title, message): (String, String)| {
            let ctx = ctx_alert.clone();
            async move {
                let _ = ctx
                    .send_command(PluginCommand::ShowAlert { title, message })
                    .await;
                Ok(())
            }
        })?,
    )?;

    // ui.error(title, msg) — show an error alert
    let ctx_error = ctx.clone();
    ui.set(
        "error",
        lua.create_async_function(move |_lua, (title, message): (String, String)| {
            let ctx = ctx_error.clone();
            async move {
                let _ = ctx
                    .send_command(PluginCommand::ShowError { title, message })
                    .await;
                Ok(())
            }
        })?,
    )?;

    // ui.show(title, text) — show a read-only text viewer
    let ctx_show = ctx.clone();
    ui.set(
        "show",
        lua.create_async_function(move |_lua, (title, text): (String, String)| {
            let ctx = ctx_show.clone();
            async move {
                let _ = ctx
                    .send_command(PluginCommand::ShowText { title, text })
                    .await;
                Ok(())
            }
        })?,
    )?;

    // ui.table(title, columns, rows) — show a table viewer
    let ctx_table = ctx.clone();
    ui.set(
        "table",
        lua.create_async_function(
            move |_lua, (title, cols_table, rows_table): (String, mlua::Table, mlua::Table)| {
                let ctx = ctx_table.clone();
                async move {
                    let columns: Vec<String> = cols_table
                        .sequence_values::<String>()
                        .collect::<Result<_, _>>()?;
                    let mut rows = Vec::new();
                    for row_val in rows_table.sequence_values::<mlua::Table>() {
                        let row_table = row_val?;
                        let row: Vec<String> = row_table
                            .sequence_values::<String>()
                            .collect::<Result<_, _>>()?;
                        rows.push(row);
                    }
                    let _ = ctx
                        .send_command(PluginCommand::ShowTable {
                            title,
                            columns,
                            rows,
                        })
                        .await;
                    Ok(())
                }
            },
        )?,
    )?;

    // ui.progress(message) — show a progress spinner
    let ctx_progress = ctx.clone();
    ui.set(
        "progress",
        lua.create_async_function(move |_lua, message: String| {
            let ctx = ctx_progress.clone();
            async move {
                let _ = ctx
                    .send_command(PluginCommand::ShowProgress { message })
                    .await;
                Ok(())
            }
        })?,
    )?;

    // ui.hide_progress() — hide the progress spinner
    let ctx_hide = ctx.clone();
    ui.set(
        "hide_progress",
        lua.create_async_function(move |_lua, ()| {
            let ctx = ctx_hide.clone();
            async move {
                let _ = ctx
                    .send_command(PluginCommand::HideProgress)
                    .await;
                Ok(())
            }
        })?,
    )?;

    // -- Panel plugin API --
    // These accumulate widgets into a thread-local list, then flush on render cycle.

    // Store panel widgets in Lua registry as a table
    let panel_widgets: mlua::Table = lua.create_table()?;
    lua.set_named_registry_value("__panel_widgets", panel_widgets)?;

    // ui.panel_clear() — clear accumulated panel widgets
    let lua_ref = lua as *const Lua;
    ui.set(
        "panel_clear",
        lua.create_function(move |lua, ()| {
            let _ = lua_ref; // ensure we use the right lua
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            widgets.clear()?;
            Ok(())
        })?,
    )?;

    // ui.panel_heading(text)
    ui.set(
        "panel_heading",
        lua.create_function(|lua, text: String| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "heading")?;
            w.set("text", text)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_text(text)
    ui.set(
        "panel_text",
        lua.create_function(|lua, text: String| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "text")?;
            w.set("text", text)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_label(text)
    ui.set(
        "panel_label",
        lua.create_function(|lua, text: String| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "label")?;
            w.set("text", text)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_separator()
    ui.set(
        "panel_separator",
        lua.create_function(|lua, ()| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "separator")?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_table(columns, rows)
    ui.set(
        "panel_table",
        lua.create_function(|lua, (cols, rows): (mlua::Table, mlua::Table)| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "table")?;
            w.set("columns", cols)?;
            w.set("rows", rows)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_progress(label, fraction, text)
    ui.set(
        "panel_progress",
        lua.create_function(|lua, (label, fraction, text): (String, f32, String)| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "progress")?;
            w.set("label", label)?;
            w.set("fraction", fraction)?;
            w.set("text", text)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_button(id, label)
    ui.set(
        "panel_button",
        lua.create_function(|lua, (id, label): (String, String)| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "button")?;
            w.set("id", id)?;
            w.set("label", label)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_kv(key, value)
    ui.set(
        "panel_kv",
        lua.create_function(|lua, (key, value): (String, String)| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "kv")?;
            w.set("key", key)?;
            w.set("value", value)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_scroll_text(lines) — scrollable monospace text area (sticks to bottom)
    ui.set(
        "panel_scroll_text",
        lua.create_function(|lua, lines: mlua::Table| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "scroll_text")?;
            w.set("lines", lines)?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_text_edit(id, hint?) — add a multiline text edit widget
    ui.set(
        "panel_text_edit",
        lua.create_function(|lua, (id, hint): (String, Option<String>)| {
            let widgets: mlua::Table = lua.named_registry_value("__panel_widgets")?;
            let len = widgets.len()? + 1;
            let w = lua.create_table()?;
            w.set("type", "text_edit")?;
            w.set("id", id)?;
            w.set("hint", hint.unwrap_or_default())?;
            widgets.set(len, w)?;
            Ok(())
        })?,
    )?;

    // ui.panel_get_text(id) — get the current text from a TextEdit widget
    let ctx_get_text = ctx.clone();
    ui.set(
        "panel_get_text",
        lua.create_async_function(move |lua, id: String| {
            let ctx = ctx_get_text.clone();
            async move {
                let resp = ctx.send_command(PluginCommand::PanelGetText { id }).await;
                match resp {
                    PluginResponse::Output(s) => Ok(Value::String(lua.create_string(&s)?)),
                    _ => Ok(Value::String(lua.create_string("")?)),
                }
            }
        })?,
    )?;

    // ui.panel_set_text(id, text) — set the text in a TextEdit widget
    let ctx_set_text = ctx.clone();
    ui.set(
        "panel_set_text",
        lua.create_async_function(move |_lua, (id, text): (String, String)| {
            let ctx = ctx_set_text.clone();
            async move {
                let _ = ctx.send_command(PluginCommand::PanelSetText { id, text }).await;
                Ok(())
            }
        })?,
    )?;

    // ui.set_refresh(seconds) — set panel refresh interval
    let ctx_refresh = ctx.clone();
    ui.set(
        "set_refresh",
        lua.create_async_function(move |_lua, seconds: f64| {
            let ctx = ctx_refresh.clone();
            async move {
                let _ = ctx.send_command(PluginCommand::PanelSetRefresh(seconds)).await;
                Ok(())
            }
        })?,
    )?;

    lua.globals().set("ui", ui)?;
    Ok(())
}

/// Collect panel widgets from the Lua registry into `Vec<PanelWidget>`.
pub fn collect_panel_widgets(lua: &Lua) -> LuaResult<Vec<PanelWidget>> {
    let widgets_table: mlua::Table = lua.named_registry_value("__panel_widgets")?;
    let mut widgets = Vec::new();

    for entry in widgets_table.sequence_values::<mlua::Table>() {
        let entry = entry?;
        let wtype: String = entry.get("type")?;
        let widget = match wtype.as_str() {
            "heading" => PanelWidget::Heading(entry.get("text")?),
            "text" => PanelWidget::Text(entry.get("text")?),
            "label" => PanelWidget::Label(entry.get("text")?),
            "separator" => PanelWidget::Separator,
            "table" => {
                let cols_table: mlua::Table = entry.get("columns")?;
                let rows_table: mlua::Table = entry.get("rows")?;
                let columns: Vec<String> = cols_table
                    .sequence_values::<String>()
                    .collect::<Result<_, _>>()?;
                let mut rows = Vec::new();
                for row_val in rows_table.sequence_values::<mlua::Table>() {
                    let row_table = row_val?;
                    let row: Vec<String> = row_table
                        .sequence_values::<String>()
                        .collect::<Result<_, _>>()?;
                    rows.push(row);
                }
                PanelWidget::Table { columns, rows }
            }
            "progress" => PanelWidget::Progress {
                label: entry.get("label")?,
                fraction: entry.get("fraction")?,
                text: entry.get("text")?,
            },
            "button" => PanelWidget::Button {
                id: entry.get("id")?,
                label: entry.get("label")?,
            },
            "kv" => PanelWidget::KeyValue {
                key: entry.get("key")?,
                value: entry.get("value")?,
            },
            "scroll_text" => {
                let lines_table: mlua::Table = entry.get("lines")?;
                let lines: Vec<String> = lines_table
                    .sequence_values::<String>()
                    .collect::<Result<_, _>>()?;
                PanelWidget::ScrollText(lines)
            }
            "text_edit" => PanelWidget::TextEdit {
                id: entry.get("id")?,
                hint: entry.get("hint").unwrap_or_default(),
            },
            _ => continue,
        };
        widgets.push(widget);
    }

    Ok(widgets)
}

/// Parse a Lua table of field descriptors into `Vec<FormField>`.
///
/// Each sub-table has:
///   `type` = "text" | "password" | "combo" | "checkbox" | "separator" | "label"
///   `name` = field name (for text/password/combo/checkbox)
///   `label` = display label
///   `default` = default value (string for text/combo, bool for checkbox)
///   `options` = list of strings (for combo)
///   `text` = label text (for label type)
fn parse_form_fields(table: &mlua::Table) -> LuaResult<Vec<FormField>> {
    let mut fields = Vec::new();

    for entry in table.sequence_values::<mlua::Table>() {
        let entry = entry?;
        let field_type: String = entry.get("type")?;

        let field = match field_type.as_str() {
            "text" => FormField::Text {
                name: entry.get("name")?,
                label: entry.get("label").unwrap_or_default(),
                default: entry.get("default").unwrap_or_default(),
            },
            "password" => FormField::Password {
                name: entry.get("name")?,
                label: entry.get("label").unwrap_or_default(),
            },
            "combo" => {
                let opts_table: mlua::Table = entry.get("options")?;
                let options: Vec<String> = opts_table
                    .sequence_values::<String>()
                    .collect::<Result<_, _>>()?;
                FormField::ComboBox {
                    name: entry.get("name")?,
                    label: entry.get("label").unwrap_or_default(),
                    options,
                    default: entry.get("default").unwrap_or_default(),
                }
            }
            "checkbox" => FormField::CheckBox {
                name: entry.get("name")?,
                label: entry.get("label").unwrap_or_default(),
                default: entry.get("default").unwrap_or(false),
            },
            "separator" => FormField::Separator,
            "label" => FormField::Label {
                text: entry.get("text").unwrap_or_default(),
            },
            other => {
                return Err(mlua::Error::runtime(format!(
                    "Unknown form field type: '{other}'"
                )));
            }
        };

        fields.push(field);
    }

    Ok(fields)
}
