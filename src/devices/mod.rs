#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
pub use win::{get_device_capabilities, get_devices_info, get_gst_device};
