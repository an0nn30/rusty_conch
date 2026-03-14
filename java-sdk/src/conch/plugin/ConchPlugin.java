package conch.plugin;

/**
 * Interface that all Conch Java plugins must implement.
 *
 * <p>The host discovers plugins by reading {@code Plugin-Class} from the
 * JAR manifest, then instantiates the class and calls these methods on a
 * dedicated plugin thread.</p>
 */
public interface ConchPlugin {
    /** Return metadata about this plugin. */
    PluginInfo getInfo();

    /** Called once after instantiation. Register menu items, subscribe to events, etc. */
    void setup();

    /** Called when a widget or menu event arrives (JSON-encoded PluginEvent). */
    void onEvent(String eventJson);

    /** Return the current widget tree as a JSON array string. */
    String render();

    /** Called before the plugin is unloaded. Release resources here. */
    void teardown();
}
