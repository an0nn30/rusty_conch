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
     * <pre>{@code {"MenuAction":{"action":"your_action_id"}}}</pre>
     *
     * <p>Typically called during {@link ConchPlugin#setup()}. Duplicate
     * registrations (same menu + label) are allowed but will create
     * multiple entries.</p>
     *
     * <p><b>Example:</b></p>
     * <pre>{@code
     * // In setup():
     * HostApi.registerMenuItem("Tools", "Analyze Code", "analyze");
     * HostApi.registerMenuItem("Tools", "Clear Cache", "clear_cache");
     *
     * // In onEvent(String eventJson):
     * if (eventJson.contains("analyze")) { ... }
     * if (eventJson.contains("clear_cache")) { ... }
     * }</pre>
     *
     * @param menu   the top-level menu name (e.g. {@code "Tools"},
     *               {@code "File"}, {@code "View"})
     * @param label  the menu item label shown to the user
     * @param action the action identifier included in the event when clicked
     */
    public static native void registerMenuItem(String menu, String label, String action);
}
