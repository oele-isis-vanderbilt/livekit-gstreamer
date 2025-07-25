use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

use display_info::DisplayInfo;
use gstreamer::{prelude::*, Device, DeviceMonitor};

use crate::{AudioCapability, MediaCapability, MediaDeviceInfo, ScreenCapability, VideoCapability};

static GLOBAL_DEVICE_MONITOR: Lazy<Arc<Mutex<DeviceMonitor>>> = Lazy::new(|| {
    let monitor = DeviceMonitor::new();
    monitor.add_filter(Some("Video/Source"), None);
    monitor.add_filter(Some("Audio/Source"), None);
    monitor.add_filter(Some("Source/Video"), None);
    monitor.add_filter(Some("Source/Audio"), None);
    if let Err(err) = monitor.start() {
        eprintln!("Failed to start global device monitor: {:?}", err);
    }
    Arc::new(Mutex::new(monitor))
});

const SUPPORTED_APIS: [&str; 4] = ["v4l2", "v4l2src", "alsa", "alsasrc"];

pub fn parse_monitors_linux() -> Vec<MediaDeviceInfo> {
    let all_monitors = DisplayInfo::all().unwrap_or_else(|_| vec![]);
    all_monitors
        .into_iter()
        .map(MediaDeviceInfo::from)
        .collect()
}

pub fn get_monitor(id_or_name: &str) -> Option<MediaDeviceInfo> {
    let all_monitors = DisplayInfo::all().unwrap_or_else(|_| vec![]);
    all_monitors
        .into_iter()
        .find(|m| m.id.to_string() == id_or_name || m.name == id_or_name)
        .map(MediaDeviceInfo::from)
}

fn get_frame_rates(display_info: &DisplayInfo) -> Vec<i32> {
    let rate = display_info.frequency;
    let mut rates = vec![rate as i32];
    if rate > 30.0 {
        rates.push(30);
    }

    rates
}

impl From<DisplayInfo> for MediaDeviceInfo {
    fn from(display_info: DisplayInfo) -> Self {
        use std::vec;
        let scale_factor = display_info.scale_factor;

        let startx = (display_info.x as f32 * scale_factor).round() as i32;
        let starty = (display_info.y as f32 * scale_factor).round() as i32;

        let endx = startx + (display_info.width as f32 * scale_factor).round() as i32;
        let endy = starty + (display_info.height as f32 * scale_factor).round() as i32;

        let actual_width = (display_info.width as f32 * scale_factor).round() as i32;
        let actual_height = (display_info.height as f32 * scale_factor).round() as i32;

        MediaDeviceInfo {
            device_path: display_info.id.clone().to_string(),
            display_name: display_info.friendly_name.clone(),
            capabilities: vec![MediaCapability::Screen(ScreenCapability {
                width: actual_width,
                height: actual_height,
                framerates: get_frame_rates(&display_info),
                codec: "video/x-raw".to_string(),
                startx,
                starty,
                endx,
                endy,
            })],
            device_class: "Screen/Source".to_string(),
        }
    }
}

pub fn get_gst_device(path: &str) -> Option<Device> {
    let device_monitor = GLOBAL_DEVICE_MONITOR.clone();
    let device_monitor = device_monitor.lock().unwrap();
    let devices = device_monitor.devices();

    devices.into_iter().find(|d| {
        let props = d.properties();

        match props {
            Some(props) => {
                // Try matching against multiple possible properties
                let candidates = [
                    props.get::<Option<String>>("api.v4l2.path"),
                    props.get::<Option<String>>("device.string"),
                    props.get::<Option<String>>("device.path"),
                ];

                // Return true if any property matches the given path
                candidates.iter().any(|res| {
                    res.clone()
                        .is_ok_and(|opt| opt.as_ref().is_some_and(|v| v.contains(path)))
                })
            }
            None => false,
        }
    })
}

pub fn get_device_capabilities(device: &Device) -> Vec<MediaCapability> {
    let caps = match device.caps() {
        Some(c) => c,
        None => return vec![],
    };

    if device.device_class() == "Video/Source" {
        caps.iter()
            .map(|s| {
                let structure = s;
                let width = structure.get::<i32>("width").unwrap();
                let height = structure.get::<i32>("height").unwrap();
                let mut framerates = vec![];
                if let Ok(framerate_fields) = structure.get::<gstreamer::List>("framerate") {
                    let frates: Vec<i32> = framerate_fields
                        .iter()
                        .map(|f| {
                            let f = f.get::<gstreamer::Fraction>();
                            match f {
                                Ok(f) => f.numer() / f.denom(),
                                Err(_) => 0,
                            }
                        })
                        .collect();
                    framerates.extend(frates);
                } else if let Ok(framerate) = structure.get::<gstreamer::Fraction>("framerate") {
                    framerates.push(framerate.numer() / framerate.denom());
                }

                let codec = structure.name().to_string();

                MediaCapability::Video(VideoCapability {
                    width,
                    height,
                    framerates,
                    codec,
                })
            })
            .collect()
    } else {
        caps.iter()
            .map(|s| {
                let structure = s;
                let channels = structure.get::<i32>("channels").unwrap_or(1);

                if let Ok(framerate_fields) = structure.get::<gstreamer::IntRange<i32>>("rate") {
                    let codec = structure.name().to_string();

                    MediaCapability::Audio(AudioCapability {
                        channels,
                        framerates: (framerate_fields.min(), framerate_fields.max()),
                        codec,
                    })
                } else {
                    MediaCapability::Audio(AudioCapability {
                        channels,
                        framerates: (0, 0),
                        codec: "audio/x-raw".to_string(),
                    })
                }
            })
            .collect()
    }
}

fn get_device_path(device: &Device) -> Option<String> {
    let props = device.properties()?;
    if device.device_class() == "Video/Source" || device.device_class() == "Source/Video" {
        props.get::<Option<String>>("api.v4l2.path").ok()?
    } else if device.device_class() == "Audio/Source" || device.device_class() == "Source/Audio" {
        // For audio devices, check for alsa path
        props.get::<Option<String>>("device.string").ok()?
    } else {
        None
    }
}

fn get_device_class(device: &Device) -> String {
    match device.device_class().as_str() {
        "Video/Source" | "Source/Video" => "Video/Source".to_string(),
        "Audio/Source" | "Source/Audio" => "Audio/Source".to_string(),
        _ => device.device_class().to_string(),
    }
}

fn confirm_supported_api(device: &Device) -> Option<bool> {
    let api = device
        .properties()
        .and_then(|props| props.get::<String>("device.api").ok())
        .unwrap_or_default();

    SUPPORTED_APIS.contains(&api.as_str()).then_some(true)
}

pub fn get_devices_info() -> Vec<MediaDeviceInfo> {
    let device_monitor = GLOBAL_DEVICE_MONITOR.clone();
    let device_monitor = device_monitor.lock().unwrap();
    let devices = device_monitor.devices();
    let mut devices = devices
        .into_iter()
        .filter_map(|d| {
            confirm_supported_api(&d)?;
            let path = get_device_path(&d)?;
            let caps = get_device_capabilities(&d);
            let display_name = d.display_name().into();
            let class = get_device_class(&d);
            Some(MediaDeviceInfo {
                device_path: path,
                display_name,
                capabilities: caps,
                device_class: class,
            })
        })
        .collect::<Vec<MediaDeviceInfo>>();

    devices.extend(parse_monitors_linux());

    devices
}
