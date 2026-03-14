-- plugin-name: Test Notifications
-- plugin-description: Test plugin for the toast notification system
-- plugin-type: panel
-- plugin-version: 0.1.0
-- plugin-location: left

function setup()
    app.log("info", "Test notifications plugin loaded")
    app.register_menu_item("Tools", "Trigger Notification", "trigger_notification")
end

function render()
    ui.panel_clear()

    ui.panel_heading("Notification Tester")
    ui.panel_label("Trigger toast notifications at each level.")
    ui.panel_separator()

    ui.panel_button("info", "Info Notification")
    ui.panel_button("success", "Success Notification")
    ui.panel_button("warning", "Warning Notification")
    ui.panel_button("error", "Error Notification")
    ui.panel_separator()

    ui.panel_button("persistent", "Persistent (no auto-dismiss)")
    ui.panel_button("quick", "Quick (1 second)")
end

function on_event(event)
    if event.kind == "menu_action" then
        if event.action == "trigger_notification" then
            app.log("info", "Triggering notification")
            app.notify("Test Notification", "Triggered from the Tools menu!", "info")
        end
    elseif event.type == "button_click" then
        if event.id == "info" then
            app.notify("Info", "This is an informational notification.", "info")
        elseif event.id == "success" then
            app.notify("Success", "Operation completed successfully!", "success")
        elseif event.id == "warning" then
            app.notify("Warning", "Something might need your attention.", "warning")
        elseif event.id == "error" then
            app.notify("Error", "Something went wrong!", "error")
        elseif event.id == "persistent" then
            app.notify("Persistent", "This stays until you dismiss it.", "info", 0)
        elseif event.id == "quick" then
            app.notify("Quick", "Gone in a flash!", "success", 1000)
        end
    end
end
