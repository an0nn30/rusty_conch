use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(java_sdk_available)");
    let sdk_jar = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../java-sdk/build/conch-plugin-sdk.jar");

    if sdk_jar.exists() {
        println!("cargo:rustc-cfg=java_sdk_available");
    } else {
        println!(
            "cargo:warning=Java SDK JAR not found at {}. \
             JVM plugin support will be disabled. \
             Build it with: make -C java-sdk build",
            sdk_jar.display()
        );
    }

    // Re-run if the JAR appears or disappears.
    println!("cargo:rerun-if-changed={}", sdk_jar.display());
}
