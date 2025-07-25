use livekit_gstreamer::get_devices_info;
use livekit_gstreamer::MediaDeviceInfo;

#[tauri::command]
pub fn get_devices() -> Vec<MediaDeviceInfo> {
    get_devices_info()
}
