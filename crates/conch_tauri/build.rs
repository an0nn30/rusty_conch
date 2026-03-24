fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Rebuild Java SDK JAR if make is available (needed by conch_plugin's include_bytes!).
    let java_sdk_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../java-sdk");
    if java_sdk_dir.join("Makefile").exists() {
        println!("cargo:rerun-if-changed=../../java-sdk/src");
        let status = std::process::Command::new("make")
            .arg("-C")
            .arg(&java_sdk_dir)
            .arg("build")
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("warning: Java SDK build exited with {s} — Java plugins may not work");
            }
            Err(e) => {
                eprintln!("warning: Could not run 'make' for Java SDK: {e} — Java plugins may not work");
            }
        }
    }

    // Embed git commit hash and timestamp for the About dialog.
    // Uses vergen-git2 for cross-platform support (works on macOS, Linux, Windows).
    // Sets VERGEN_GIT_SHA, VERGEN_GIT_COMMIT_TIMESTAMP, etc.
    let git = vergen_git2::Git2Builder::all_git()?;
    vergen_git2::Emitter::default()
        .add_instructions(&git)?
        .emit()?;

    tauri_build::build();
    Ok(())
}
