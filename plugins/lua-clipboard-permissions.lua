-- plugin-name: Clipboard Permissions Demo
-- plugin-description: Example plugin that declares clipboard permissions and uses clipboard APIs
-- plugin-version: 0.1.0
-- plugin-type: action
-- plugin-api: ^1.0
-- plugin-permissions: ui.menu, ui.notify, clipboard.read, clipboard.write

local ACTION_SHOW = "clipboard_demo_show"
local ACTION_UPPER = "clipboard_demo_upper"

local function preview(text, max_len)
    if text == nil then
        return "(clipboard is empty)"
    end
    if #text <= max_len then
        return text
    end
    return string.sub(text, 1, max_len) .. "..."
end

function setup()
    app.log("info", "Clipboard Permissions Demo loaded")
    app.register_menu_item("Tools", "Clipboard: Show Preview", ACTION_SHOW)
    app.register_menu_item("Tools", "Clipboard: UPPERCASE Selection", ACTION_UPPER)
end

function on_event(event)
    if type(event) ~= "table" then
        return
    end

    if event.action == ACTION_SHOW then
        local text = app.clipboard_get()
        app.notify("Clipboard Preview", preview(text, 120), "info", 3500)
        return
    end

    if event.action == ACTION_UPPER then
        local text = app.clipboard_get()
        if text == nil or text == "" then
            app.notify("Clipboard", "Nothing to transform", "warn", 2500)
            return
        end
        app.clipboard(string.upper(text))
        app.notify("Clipboard", "Transformed text copied back to clipboard", "success", 2500)
    end
end
