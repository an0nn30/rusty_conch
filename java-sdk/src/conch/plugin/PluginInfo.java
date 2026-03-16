package conch.plugin;

/**
 * Metadata describing a Conch plugin.
 *
 * <p>Returned by {@link ConchPlugin#getInfo()} during discovery and loading.
 * The fields are displayed in the Plugin Manager UI and used by the host to
 * determine where to render the plugin's panel (if any).</p>
 *
 * <h2>Plugin Types</h2>
 * <ul>
 *   <li>{@code "panel"} — the plugin renders a widget panel at the specified
 *       {@link #panelLocation}. The host calls {@link ConchPlugin#render()}
 *       every frame.</li>
 *   <li>{@code "action"} — the plugin runs in the background and interacts
 *       only through menu items, events, and host API calls. No panel is
 *       rendered.</li>
 * </ul>
 *
 * <h2>Panel Locations</h2>
 * <ul>
 *   <li>{@code "left"} — left sidebar (e.g. session tree)</li>
 *   <li>{@code "right"} — right sidebar</li>
 *   <li>{@code "bottom"} — bottom panel (e.g. logs, output)</li>
 *   <li>{@code "none"} — no panel (for action plugins)</li>
 * </ul>
 *
 * <h2>Example</h2>
 * <pre>{@code
 * // Action plugin (no panel):
 * new PluginInfo("My Tool", "Does useful things", "1.0.0");
 *
 * // Panel plugin (right sidebar):
 * new PluginInfo("File Browser", "Browse remote files", "0.2.0", "panel", "right");
 * }</pre>
 *
 * @see ConchPlugin#getInfo()
 */
public class PluginInfo {

    /**
     * Display name of the plugin.
     *
     * <p>Shown in the Plugin Manager and used as the plugin's unique
     * identifier. Must be non-null and non-empty.</p>
     */
    public final String name;

    /**
     * Short description of what the plugin does.
     *
     * <p>Shown in the Plugin Manager UI beneath the plugin name.</p>
     */
    public final String description;

    /**
     * Semantic version string (e.g. {@code "1.0.0"}, {@code "0.3.2-beta"}).
     */
    public final String version;

    /**
     * Plugin type: {@code "panel"} or {@code "action"}.
     *
     * <p>Panel plugins render widgets via {@link ConchPlugin#render()}.
     * Action plugins run in the background without a UI panel.</p>
     */
    public final String pluginType;

    /**
     * Where to display the plugin's panel: {@code "left"}, {@code "right"},
     * {@code "bottom"}, or {@code "none"}.
     *
     * <p>Ignored for action plugins.</p>
     */
    public final String panelLocation;

    /**
     * Create plugin metadata with full control over type and panel location.
     *
     * @param name          display name (unique identifier)
     * @param description   short description
     * @param version       semantic version string
     * @param pluginType    {@code "panel"} or {@code "action"}
     * @param panelLocation {@code "left"}, {@code "right"}, {@code "bottom"},
     *                      or {@code "none"}
     */
    public PluginInfo(String name, String description, String version,
                      String pluginType, String panelLocation) {
        this.name = name;
        this.description = description;
        this.version = version;
        this.pluginType = pluginType;
        this.panelLocation = panelLocation;
    }

    /**
     * Create metadata for an action plugin (no panel).
     *
     * <p>Equivalent to
     * {@code new PluginInfo(name, description, version, "action", "none")}.</p>
     *
     * @param name        display name
     * @param description short description
     * @param version     semantic version string
     */
    public PluginInfo(String name, String description, String version) {
        this(name, description, version, "action", "none");
    }
}
