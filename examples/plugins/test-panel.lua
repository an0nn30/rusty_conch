-- plugin-name: Test Panel
-- plugin-description: Trivial Lua panel for testing widget sugar
-- plugin-type: panel
-- plugin-version: 0.1.0
-- plugin-location: left

local counter = 0
local checkbox_state = false
local combo_value = "option_a"
local text_value = ""

function setup()
    app.log("info", "Test panel plugin loaded")
end

function render()
    ui.panel_clear()

    ui.panel_heading("Test Panel")
    ui.panel_label("A trivial panel to validate all widget types.")
    ui.panel_separator()

    -- Counter button
    ui.panel_kv("Counter:", tostring(counter))
    ui.panel_button("increment", "Increment")
    ui.panel_button("reset", "Reset Counter")

    ui.panel_separator()

    -- Interactive widgets
    ui.panel_heading("Interactive Widgets")
    ui.panel_checkbox("toggle", "Enable feature", checkbox_state)
    ui.panel_combobox("selector", combo_value, {
        { value = "option_a", label = "Option A" },
        { value = "option_b", label = "Option B" },
        { value = "option_c", label = "Option C" },
    })
    ui.panel_text_input("input", text_value, "Type something...")

    ui.panel_separator()

    -- Display widgets
    ui.panel_heading("Display Widgets")
    ui.panel_text("Monospace text block")
    ui.panel_badge("active", "success")
    ui.panel_badge("warning", "warn")
    ui.panel_progress("load", 0.65, "65%")
    ui.panel_icon_label("server", "Server Status")
    ui.panel_spacer(10)
    ui.panel_label("Muted text", "muted")

    ui.panel_separator()

    -- Table
    ui.panel_heading("Sample Table")
    ui.panel_table({"Name", "Value", "Status"}, {
        {"CPU", "45%", "ok"},
        {"Memory", "72%", "warning"},
        {"Disk", "89%", "critical"},
    })
end

function on_event(event)
    if event.type == "button_click" then
        if event.id == "increment" then
            counter = counter + 1
        elseif event.id == "reset" then
            counter = 0
        end
    elseif event.type == "checkbox_changed" then
        if event.id == "toggle" then
            checkbox_state = event.checked
        end
    elseif event.type == "combobox_changed" then
        if event.id == "selector" then
            combo_value = event.value
        end
    elseif event.type == "text_input_changed" then
        if event.id == "input" then
            text_value = event.value
        end
    end
end
