package conch.plugin.hello;

import conch.plugin.ConchPlugin;
import conch.plugin.HostApi;
import conch.plugin.PluginInfo;

/**
 * Minimal Java plugin for Conch — registers a menu item that logs a message.
 */
public class HelloPlugin implements ConchPlugin {

    @Override
    public PluginInfo getInfo() {
        return new PluginInfo(
            "Hello Java",
            "Test Java plugin - registers a menu item",
            "0.1.0",
            "action",
            "none"
        );
    }

    @Override
    public void setup() {
        HostApi.info("Hello Java plugin: setup");
        HostApi.registerMenuItem("Tools", "Java: Say Hello", "say_hello");
    }

    @Override
    public void onEvent(String eventJson) {
        if (eventJson.contains("say_hello")) {
            HostApi.info("Hello from Java plugin! The menu item was clicked.");
        }
    }

    @Override
    public String render() {
        return "[]";
    }

    @Override
    public void teardown() {
        HostApi.info("Hello Java plugin: teardown");
    }
}
