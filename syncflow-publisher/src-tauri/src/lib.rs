// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod devices;

use devices::get_devices;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            livekit_gstreamer::initialize_gstreamer();
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_devices])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
