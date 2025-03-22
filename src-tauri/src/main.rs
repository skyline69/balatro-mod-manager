// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    // #[cfg(debug_assertions)]
    // {
    //     std::env::set_var("RUST_LOG", "debug");
    //     if let Err(e) = pretty_env_logger::try_init() {
    //         eprintln!("Failed to initialize logger: {}", e);
    //     }
    // }
    let _ = fix_path_env::fix();
    if let Err(e) = bmm_lib::logging::init_logger() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");

    balatro_mod_manager_lib::run();

    #[cfg(target_os = "linux")]
    std::env::remove_var("WEBKIT_DISABLE_COMPOSITING_MODE");

    log::logger().flush();
}
