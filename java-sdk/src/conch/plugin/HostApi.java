package conch.plugin;

/**
 * Host API — static native methods that plugins call to interact with Conch.
 *
 * <p>These methods are backed by Rust via JNI. They are registered by the host
 * before any plugin code runs, so they are always available.</p>
 */
public class HostApi {
    private HostApi() {} // Static-only class.

    /**
     * Log a message at the given level.
     *
     * @param level 0=trace, 1=debug, 2=info, 3=warn, 4=error
     * @param message the log message
     */
    public static native void log(int level, String message);

    /** Convenience: log at INFO level. */
    public static void info(String message) { log(2, message); }

    /** Convenience: log at WARN level. */
    public static void warn(String message) { log(3, message); }

    /** Convenience: log at ERROR level. */
    public static void error(String message) { log(4, message); }

    /**
     * Register a menu item in the app's menu bar.
     *
     * <p>When the user clicks the item, the plugin receives a
     * {@code MenuAction} event with the given {@code action} string.</p>
     *
     * @param menu   top-level menu name (e.g. "Tools")
     * @param label  menu item label
     * @param action action identifier sent back as an event
     */
    public static native void registerMenuItem(String menu, String label, String action);
}
