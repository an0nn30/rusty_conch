package conch.plugin;

/**
 * Metadata describing a plugin.
 */
public class PluginInfo {
    public final String name;
    public final String description;
    public final String version;
    public final String pluginType;     // "panel" or "action"
    public final String panelLocation;  // "left", "right", "bottom", or "none"

    public PluginInfo(String name, String description, String version,
                      String pluginType, String panelLocation) {
        this.name = name;
        this.description = description;
        this.version = version;
        this.pluginType = pluginType;
        this.panelLocation = panelLocation;
    }

    /** Convenience constructor for action plugins (no panel). */
    public PluginInfo(String name, String description, String version) {
        this(name, description, version, "action", "none");
    }
}
