use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

use gstreamer::{prelude::*, Device, DeviceMonitor};

use crate::{AudioCapability, MediaCapability, MediaDeviceInfo, VideoCapability};

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

const SUPPORTED_APIS: [&str; 5] = [
    "wasapi",
    "mediafoundation",
    "directshow",
    "dshow",
    "wasapi2",
];

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
                    props.get::<Option<String>>("object.path"),
                    props.get::<Option<String>>("device.path"),
                    props.get::<Option<String>>("device.id"),
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

    if device.device_class() == "Video/Source" || device.device_class() == "Source/Video" {
        #[allow(clippy::unnecessary_filter_map)]
        caps.iter()
            .filter_map(|structure| {
                let width = structure.get::<i32>("width").ok();
                let height = structure.get::<i32>("height").ok();

                let mut framerates = vec![];
                if let Ok(framerate_list) = structure.get::<gstreamer::List>("framerate") {
                    for val in framerate_list.iter() {
                        if let Ok(frac) = val.get::<gstreamer::Fraction>() {
                            framerates.push(frac.numer() as f64 / frac.denom() as f64);
                        }
                    }
                } else if let Ok(framerate) = structure.get::<gstreamer::Fraction>("framerate") {
                    framerates.push(framerate.numer() as f64 / framerate.denom() as f64);
                }

                let codec = structure.name().to_string();

                Some(MediaCapability::Video(VideoCapability {
                    width: width.unwrap_or(0),
                    height: height.unwrap_or(0),
                    framerates: framerates.iter().map(|&f| f as i32).collect(),
                    codec,
                }))
            })
            .collect()
    } else {
        #[allow(clippy::unnecessary_filter_map)]
        caps.iter()
            .filter_map(|structure| {
                let channels = structure.get::<i32>("channels").unwrap_or(0);

                let mut rates = vec![];
                // Try to get a list of rates
                if let Ok(rate_list) = structure.get::<gstreamer::List>("rate") {
                    for val in rate_list.iter() {
                        if let Ok(rate) = val.get::<i32>() {
                            rates.push(rate);
                        }
                    }
                } else if let Ok(rate) = structure.get::<i32>("rate") {
                    rates.push(rate);
                } else if let Ok(rate_range) = structure.get::<gstreamer::IntRange<i32>>("rate") {
                    rates.push(rate_range.min());
                    rates.push(rate_range.max());
                }

                let codec = structure.name().to_string();

                Some(MediaCapability::Audio(AudioCapability {
                    channels,
                    framerates: if rates.is_empty() {
                        (0, 0)
                    } else {
                        (
                            *rates.iter().min().unwrap_or(&0),
                            *rates.iter().max().unwrap_or(&0),
                        )
                    },
                    codec,
                }))
            })
            .collect()
    }
}

fn get_device_path(device: &Device) -> Option<String> {
    let props = device.properties()?;
    let path = props
        .get("device.path")
        .unwrap_or_else(|_| props.get("device.id").ok().flatten());

    #[allow(clippy::manual_unwrap_or_default)]
    path.or_else(|| match props.get::<Option<String>>("device.path") {
        Ok(path) => path,
        Err(_) => None,
    })
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
    devices
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
        .collect()
}
