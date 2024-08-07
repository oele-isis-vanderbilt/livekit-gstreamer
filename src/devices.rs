use gstreamer::glib::property::PropertyGet;
use gstreamer::prelude::DeviceMonitorExt;
use gstreamer::prelude::*;
use gstreamer::DeviceMonitor;
use tokio::sync::broadcast::channel;
// use crate::models::VideoDevice;

pub fn list_video_devices() -> Vec<String> {
    vec![]
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_list_audio_devices() {
//         let devices = list_video_devices();
//         println!("Devices: {:?}", devices);
//         assert!(devices.len() > 0);
//     }
// }
