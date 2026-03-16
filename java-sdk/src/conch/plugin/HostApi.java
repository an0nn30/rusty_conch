package conch.plugin;

/**
 * Host API for Conch plugins.
 *
 * <p>Provides static methods that plugins call to interact with the Conch
 * terminal emulator. All methods are backed by native (Rust/JNI)
 * implementations that are registered by the host before any plugin code
 * runs.</p>
 *
 * <p>These methods are <b>thread-safe</b> and can be called from any thread,
 * though they are typically called from the plugin's dedicated thread.</p>
 *
 * <h2>Available Operations</h2>
 * <ul>
 *   <li><b>Logging</b> — {@link #log}, {@link #info}, {@link #warn},
 *       {@link #error}, {@link #debug}, {@link #trace}</li>
 *   <li><b>Menu items</b> — {@link #registerMenuItem}</li>
 * </ul>
 *
 * <h2>Example</h2>
 * <pre>{@code
 * public void setup() {
 *     HostApi.info("Plugin starting up...");
 *     HostApi.registerMenuItem("Tools", "Run Analysis", "run_analysis");
 * }
 *
 * public void onEvent(String eventJson) {
 *     if (eventJson.contains("run_analysis")) {
 *         HostApi.info("Running analysis...");
 *         // ... do work ...
 *         HostApi.info("Analysis complete.");
 *     }
 * }
 * }</pre>
 *
 * @see ConchPlugin
 */
public class HostApi {
    private HostApi() {} // Static-only class — not instantiable.

    // -----------------------------------------------------------------------
    // Logging
    // -----------------------------------------------------------------------

    /** Log level constant: trace (most verbose). */
    public static final int LOG_TRACE = 0;
    /** Log level constant: debug. */
    public static final int LOG_DEBUG = 1;
    /** Log level constant: info. */
    public static final int LOG_INFO = 2;
    /** Log level constant: warn. */
    public static final int LOG_WARN = 3;
    /** Log level constant: error (least verbose). */
    public static final int LOG_ERROR = 4;

    /**
     * Log a message at the specified level.
     *
     * <p>Messages appear in Conch's log output (visible when running with
     * {@code RUST_LOG=info} or similar).</p>
     *
     * @param level one of {@link #LOG_TRACE}, {@link #LOG_DEBUG},
     *              {@link #LOG_INFO}, {@link #LOG_WARN}, {@link #LOG_ERROR}
     * @param message the message to log (must not be null)
     */
    public static native void log(int level, String message);

    /**
     * Log a message at TRACE level.
     *
     * @param message the message to log
     */
    public static void trace(String message) { log(LOG_TRACE, message); }

    /**
     * Log a message at DEBUG level.
     *
     * @param message the message to log
     */
    public static void debug(String message) { log(LOG_DEBUG, message); }

    /**
     * Log a message at INFO level.
     *
     * @param message the message to log
     */
    public static void info(String message) { log(LOG_INFO, message); }

    /**
     * Log a message at WARN level.
     *
     * @param message the message to log
     */
    public static void warn(String message) { log(LOG_WARN, message); }

    /**
     * Log a message at ERROR level.
     *
     * @param message the message to log
     */
    public static void error(String message) { log(LOG_ERROR, message); }

    // -----------------------------------------------------------------------
    // Menu Items
    // -----------------------------------------------------------------------

    /**
     * Register a menu item in Conch's menu bar.
     *
     * <p>The item appears under the specified top-level menu. When the user
     * clicks it, the plugin's {@link ConchPlugin#onEvent(String)} is called
     * with a JSON event containing the {@code action} string:</p>
     *
     * <pre>{@code {"kind":"menu_action","action":"your_action_id"}}</pre>
     *
     * <p>Typically called during {@link ConchPlugin#setup()}.</p>
     *
     * @param menu   the top-level menu name (e.g. {@code "Tools"})
     * @param label  the menu item label shown to the user
     * @param action the action identifier included in the event when clicked
     */
    public static native void registerMenuItem(String menu, String label, String action);

    /**
     * Register a menu item with a keyboard shortcut.
     *
     * @param menu    the top-level menu name
     * @param label   the menu item label
     * @param action  the action identifier
     * @param keybind keyboard shortcut (e.g. {@code "cmd+shift+j"})
     */
    public static native void registerMenuItemWithKeybind(String menu, String label, String action, String keybind);

    // -----------------------------------------------------------------------
    // Notifications
    // -----------------------------------------------------------------------

    /**
     * Show a toast notification.
     *
     * @param title      notification title (may be null)
     * @param body       notification body text
     * @param level      one of {@code "info"}, {@code "success"},
     *                   {@code "warn"}, {@code "error"}
     * @param durationMs display duration in milliseconds (0 = persistent,
     *                   -1 = default 5 seconds)
     */
    public static native void notify(String title, String body, String level, int durationMs);

    /**
     * Show a toast notification with default duration.
     *
     * @param title notification title
     * @param body  notification body text
     * @param level notification level
     */
    public static void notify(String title, String body, String level) {
        notify(title, body, level, -1);
    }

    // -----------------------------------------------------------------------
    // Status Bar
    // -----------------------------------------------------------------------

    /**
     * Update the global status bar.
     *
     * @param text     status text to display (null to clear)
     * @param level    0=info, 1=warn, 2=error, 3=success
     * @param progress 0.0–1.0 to show a progress bar, or negative to hide it
     */
    public static native void setStatus(String text, int level, float progress);

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    /**
     * Set the system clipboard contents.
     *
     * @param text the text to copy to the clipboard
     */
    public static native void clipboardSet(String text);

    /**
     * Get the system clipboard contents.
     *
     * @return the clipboard text, or null if unavailable
     */
    public static native String clipboardGet();

    // -----------------------------------------------------------------------
    // Plugin Config (persistent key/value storage)
    // -----------------------------------------------------------------------

    /**
     * Read a persisted config value for this plugin.
     *
     * <p>Config is stored per-plugin in
     * {@code ~/.config/conch/plugins/<plugin-name>/<key>.json}.</p>
     *
     * @param key the config key
     * @return the JSON value string, or null if not set
     */
    public static native String getConfig(String key);

    /**
     * Write a persisted config value for this plugin.
     *
     * @param key   the config key
     * @param value the JSON value string (null to delete)
     */
    public static native void setConfig(String key, String value);

    // -----------------------------------------------------------------------
    // Dialogs
    // -----------------------------------------------------------------------

    /**
     * Show a blocking text prompt dialog and wait for user input.
     *
     * <p>The dialog blocks the plugin thread until the user submits or
     * cancels. Returns the entered text, or null if cancelled.</p>
     *
     * @param message      the prompt message
     * @param defaultValue pre-filled default value (use "" for empty)
     * @return the entered text, or null if cancelled
     */
    public static native String prompt(String message, String defaultValue);

    /**
     * Show a blocking text prompt with no default value.
     *
     * @param message the prompt message
     * @return the entered text, or null if cancelled
     */
    public static String prompt(String message) {
        return prompt(message, "");
    }

    /**
     * Show a blocking confirmation dialog (OK / Cancel).
     *
     * @param message the confirmation message
     * @return true if the user confirmed, false if cancelled
     */
    public static native boolean confirm(String message);

    /**
     * Show a blocking alert dialog with an OK button.
     *
     * @param title   the alert title
     * @param message the alert message
     */
    public static native void alert(String title, String message);

    /**
     * Show a blocking error dialog with an OK button.
     *
     * @param title   the error title
     * @param message the error message
     */
    public static native void showError(String title, String message);

    // -----------------------------------------------------------------------
    // Forms
    // -----------------------------------------------------------------------

    /**
     * Show a blocking multi-field form dialog.
     *
     * <p>The form descriptor is a JSON string describing the title and fields.
     * Supported field types: {@code text}, {@code password}, {@code number},
     * {@code combo} (dropdown), {@code checkbox}, {@code host_port},
     * {@code file_picker}, {@code collapsible}, {@code separator},
     * {@code label}.</p>
     *
     * <p><b>Example:</b></p>
     * <pre>{@code
     * String formJson = """
     *     {
     *         "title": "New Connection",
     *         "fields": [
     *             {"type": "text", "id": "host", "label": "Hostname", "hint": "e.g. server.example.com"},
     *             {"type": "number", "id": "port", "label": "Port", "value": 22},
     *             {"type": "text", "id": "user", "label": "Username"},
     *             {"type": "password", "id": "pass", "label": "Password"},
     *             {"type": "combo", "id": "auth", "label": "Auth Method",
     *              "options": ["password", "key", "agent"], "value": "password"},
     *             {"type": "checkbox", "id": "save", "label": "Save credentials", "value": true}
     *         ]
     *     }
     *     """;
     * String result = HostApi.showForm(formJson);
     * if (result != null) {
     *     // result is a JSON object: {"host":"...", "port":22, "user":"...", ...}
     *     JsonObject obj = JsonParser.parseString(result).getAsJsonObject();
     *     String host = obj.get("host").getAsString();
     * }
     * }</pre>
     *
     * @param formDescriptorJson JSON string with {@code title} and {@code fields} array
     * @return JSON object with field values keyed by id, or null if cancelled
     */
    public static native String showForm(String formDescriptorJson);

    // -----------------------------------------------------------------------
    // Inter-Plugin Communication
    // -----------------------------------------------------------------------

    /**
     * Subscribe to bus events from other plugins.
     *
     * <p>When a matching event is published, the plugin's
     * {@link ConchPlugin#onEvent(String)} receives it as a
     * {@code bus_event} JSON envelope.</p>
     *
     * @param eventType the event type to subscribe to (e.g. {@code "ssh.connected"})
     */
    public static native void subscribe(String eventType);

    /**
     * Publish an event on the plugin bus.
     *
     * @param eventType the event type string
     * @param dataJson  JSON-encoded event data
     */
    public static native void publishEvent(String eventType, String dataJson);

    // -----------------------------------------------------------------------
    // Terminal / Session
    // -----------------------------------------------------------------------

    /**
     * Write text to the focused window's active terminal session.
     *
     * <p>The text is sent as raw bytes to the PTY. Include {@code "\n"} to
     * simulate pressing Enter.</p>
     *
     * @param text the text to write (e.g. {@code "ls -la\n"})
     */
    public static native void writeToPty(String text);

    /**
     * Open a new local shell tab in the focused window.
     *
     * @param command optional command to run in the new tab (null for none)
     * @param plain   if true, use the OS default shell instead of the
     *                configured terminal.shell (avoids nesting tmux, etc.)
     */
    public static native void newTab(String command, boolean plain);

    /**
     * Open a new tab with default shell configuration.
     */
    public static void newTab() {
        newTab(null, false);
    }

    /**
     * Open a new plain shell tab and run a command.
     *
     * @param command the command to execute in the new tab
     */
    public static void newPlainTab(String command) {
        newTab(command, true);
    }
}
