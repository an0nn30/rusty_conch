//! Conch Mobile — iOS SSH client.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running Conch Mobile");
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_module_loads() {
        // Smoke test — verifies the crate compiles and links.
        assert!(true);
    }
}
