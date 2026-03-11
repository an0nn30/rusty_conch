-- plugin-name: Notes
-- plugin-description: Simple scratchpad for quick notes
-- plugin-type: session-panel
-- plugin-version: 1.0.0
-- plugin-keybind: open_panel = cmd+shift+n | Toggle Notes Panel

function setup()
    -- nothing to initialise
end

function render()
    ui.panel_text_edit("notes", "Type your notes here...")
end
