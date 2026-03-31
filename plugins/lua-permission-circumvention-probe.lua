-- plugin-name: Permission Circumvention Probe
-- plugin-description: Intentionally attempts forbidden operations to verify permission enforcement
-- plugin-version: 0.1.0
-- plugin-type: action
-- plugin-api: ^1.0
-- plugin-permissions: ui.menu, ui.notify

local ACTION_RUN = "perm_probe.run"

function setup()
    app.register_menu_item("Tools", "Permissions: Run Circumvention Probe", ACTION_RUN)
    app.log("info", "Permission Circumvention Probe loaded")
end

local function line(label, value)
    return label .. ": " .. tostring(value)
end

function on_event(event)
    if type(event) ~= "table" or event.action ~= ACTION_RUN then
        return
    end

    -- Attempt clipboard read/write without clipboard permissions.
    local clip_before = app.clipboard_get()
    app.clipboard("probe-write")

    -- Attempt local command execution without session.exec permission.
    local exec_result = session.exec("echo probe-exec")

    -- Attempt network operations without net permissions.
    local resolved = net.resolve("localhost")
    local scanned = net.scan("127.0.0.1", {22, 80, 443}, 200, 8)

    -- Attempt config write without config.write permission.
    app.set_config("permission_probe", "blocked")

    local body = table.concat({
        line("clipboard_get result", clip_before ~= nil and "value returned" or "nil"),
        line("session.exec status", exec_result and exec_result.status or "nil"),
        line("session.exec stderr", exec_result and exec_result.stderr or "nil"),
        line("net.resolve count", resolved and #resolved or 0),
        line("net.scan count", scanned and #scanned or 0),
    }, "\n")

    app.notify("Permission Probe Results", body, "info", 5000)
end
